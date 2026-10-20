use std::path::Path;

pub mod auth;
pub mod auth_methods;
#[cfg(feature = "clustering")]
pub mod clustering;
pub mod config;
pub mod liveness;
pub mod persist;
pub mod private_dir;
pub mod protocol;
pub mod revocation;
pub mod server;
pub mod tenant_id;

pub use auth::{AuthError, AuthResult, Permission, PermissionSet, TenantAuthContext};
pub use server::message_router::{MessageRouter, RoutingDecision};
pub use server::WssServer;
pub use server::WssServerConfig;

/// A service: a memory-mapped file of bare bytes, from offset 0. Commy writes nothing into it
/// (no header, no allocator state); what the bytes mean is up to the clients that map it.
pub struct Service {
    mmap: memmap2::MmapMut,
    path: String,
}

impl Service {
    /// Map an existing, writable service file.
    pub fn new(service_name: &str) -> Self {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(service_name)
            .unwrap();
        let mmap = unsafe { memmap2::MmapMut::map_mut(&file) }.unwrap();
        Service {
            mmap,
            path: service_name.to_string(),
        }
    }

    /// Create or open a service backed by a memory-mapped file.
    /// Creates the file with `initial_size` zero bytes if it does not exist yet.
    pub fn open_or_create(file_path: &str, initial_size: usize) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)?;
        if file.metadata()?.len() == 0 {
            file.set_len(initial_size as u64)?;
        }
        let mmap = unsafe { memmap2::MmapMut::map_mut(&file) }?;
        Ok(Service {
            mmap,
            path: file_path.to_string(),
        })
    }

    /// Size of the file in bytes.
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// True for an empty mapping (never the case for a service created here).
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// The file's bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap
    }

    /// The file's bytes, writable.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.mmap
    }

    /// Flush the mapping to the file.
    pub fn flush(&self) -> std::io::Result<()> {
        self.mmap.flush()
    }

    /// Where the file lives.
    pub fn path(&self) -> &Path {
        Path::new(&self.path)
    }
}

pub struct Tenant {
    /// Unique tenant identifier
    id: String,

    /// Services managed by this tenant
    services: std::collections::HashMap<String, Service>,

    /// Authentication context for this tenant
    auth_context: std::sync::Arc<tokio::sync::RwLock<TenantAuthContext>>,
}

impl Tenant {
    /// Create a new tenant with authentication support
    pub fn new(tenant_id: String, auth_context: TenantAuthContext) -> Self {
        Tenant {
            id: tenant_id,
            services: std::collections::HashMap::new(),
            auth_context: std::sync::Arc::new(tokio::sync::RwLock::new(auth_context)),
        }
    }

    /// Get the tenant ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get a reference to the authentication context
    pub fn auth_context(&self) -> &std::sync::Arc<tokio::sync::RwLock<TenantAuthContext>> {
        &self.auth_context
    }

    pub fn get_service(&mut self, service_name: &str) -> &mut Service {
        self.services
            .entry(service_name.to_string())
            .or_insert_with(move || Service::new(service_name))
    }

    /// Register a service backed by a given file on disk (creates the file, `size` bytes, if it
    /// doesn't exist). The caller has already validated and page-rounded `size`.
    pub fn register_service(
        &mut self,
        service_name: &str,
        file_path: &str,
        size: usize,
    ) -> std::io::Result<()> {
        let service = Service::open_or_create(file_path, size)?;
        self.services.insert(service_name.to_string(), service);
        Ok(())
    }

    /// Forget a registered service (its mapping is dropped). Returns whether it existed.
    pub fn remove_service(&mut self, service_name: &str) -> bool {
        self.services.remove(service_name).is_some()
    }

    /// Get an immutable reference to a registered service by its logical name.
    pub fn get_service_by_name(&self, service_name: &str) -> Option<&Service> {
        self.services.get(service_name)
    }

    /// Get a mutable reference to a registered service by its logical name.
    pub fn get_service_mut_by_name(&mut self, service_name: &str) -> Option<&mut Service> {
        self.services.get_mut(service_name)
    }
}

/// A file prepared for a resize: created, zero filled, not live; only the requester may write it.
pub struct PreparedFile {
    pub service_id: String,
    pub requester_session: String,
    pub path: String,
    pub size: u64,
}

/// Server-side record for a service.
/// Only the Server knows the (tenant_name, service_name) → file mapping — clients only
/// know the opaque service_id returned at creation time.
pub struct ServiceRecord {
    pub tenant_name: String,
    pub service_name: String,
    pub file_path: String,
    /// The id range frames use for this service's file; assigned by the server at creation.
    pub file_id: u32,
    /// Whether an admin has opened this service to local mapping. Off by default: the server
    /// cannot see stores into a mapping, so only OS file permissions guard it.
    pub trusted_local: bool,
    /// The service's current sequence number (a snapshot carries it; C3 will advance it).
    pub seq: u64,
    /// How many times the file has been rotated.
    pub generation: u64,
}

pub struct Server {
    pub tenants: std::collections::HashMap<String, Tenant>,
    #[allow(dead_code)]
    servers: Vec<Server>,
    #[allow(dead_code)]
    clients: Vec<Client>,
    /// Maps service_id → ServiceRecord (only Server knows this mapping)
    pub service_registry: std::collections::HashMap<String, ServiceRecord>,
    /// Development authentication: accepts any credential and grants admin. Off unless the
    /// operator sets `COMMY_ALLOW_DEV_AUTH=1`. Never enable it on a reachable server.
    pub dev_auth: bool,
    /// Largest service file (bytes) a client may ask for.
    pub max_service_size: u64,
    /// How many audit events to keep (`COMMY_AUDIT_MAX_EVENTS`); `None` keeps the store's default.
    pub audit_max_events: Option<u64>,
    /// The next `file_id` to hand out.
    pub next_file_id: u32,
    /// Who is on which file, for range forwarding.
    pub forwarder: server::forwarding::Registry,
    /// Running rotations and their rate limits.
    pub rotations: server::rotation::Rotations,
    /// Service-scoped grants (role-system, in memory).
    pub grants: Box<dyn server::grants::GrantStore>,
    /// Files that were rotated away; frames for them are refused as `INVALID_REQUEST`.
    pub retired_files: std::collections::HashSet<u32>,
    /// Files prepared for a resize and not yet live, by `file_id`.
    pub prepared: std::collections::HashMap<u32, PreparedFile>,
    /// How long the requester has to fill a prepared file (default 30 s, `COMMY_FILL_TIMEOUT_MS`).
    pub fill_timeout: std::time::Duration,
    /// How long a resize waits for every SDK to drain (default 5 s, `COMMY_DRAIN_TIMEOUT_MS`).
    pub drain_timeout: std::time::Duration,
    /// Least time between two rotations of one service.
    pub rotation_min_interval: std::time::Duration,
    /// Rotation requests, grants and refusals, oldest first (capped).
    audit_log: std::sync::Mutex<Vec<String>>,
    /// Audit events that could not be written to the durable log.
    audit_failures: std::sync::atomic::AtomicU64,
    /// Durable state (service registry and grants), if enabled. Off for `Server::new()` unless
    /// `enable_persistence` is called; the binary turns it on.
    persistence: Option<persist::Persistence>,
    /// Verifies the credentials in `Authenticate` (production). `None` with `dev_auth` off means
    /// nobody can authenticate. Development authentication ignores it.
    pub verifier: Option<std::sync::Arc<dyn server::verify::CredentialVerifier>>,
    /// Serialises "record a grant change, then apply it" so the log order is the applied order.
    grant_lock: std::sync::Mutex<()>,
}

/// What loading the stored state found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateReport {
    pub services_loaded: usize,
    /// Stored services whose file is gone; they are skipped, not deleted.
    pub services_missing_file: usize,
    pub grant_ops_replayed: usize,
    /// Stored grant operations that could not be applied (skipped).
    pub grant_ops_skipped: usize,
}

/// Page size service files are rounded up to.
pub const PAGE_SIZE: u64 = 4096;

/// Default largest service file a client may ask for: 10 MiB.
pub const DEFAULT_MAX_SERVICE_SIZE: u64 = 10 * 1024 * 1024;

/// Interim way to set the maximum service size (bytes) until config-file support exists.
pub const MAX_SERVICE_SIZE_ENV: &str = "COMMY_MAX_SERVICE_SIZE";

/// Why a requested service size was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSizeError {
    /// A zero-length file cannot be mapped.
    Zero,
    /// The (page-rounded) size is above the server maximum.
    TooLarge,
}

/// The file size to create for a requested size: at least one byte, rounded up to whole pages,
/// and no more than `max` (itself rounded up to whole pages).
pub fn service_file_size(requested: u64, max: u64) -> Result<u64, ServiceSizeError> {
    if requested == 0 {
        return Err(ServiceSizeError::Zero);
    }
    let round = |n: u64| n.checked_add(PAGE_SIZE - 1).map(|v| v / PAGE_SIZE * PAGE_SIZE);
    let rounded = round(requested).ok_or(ServiceSizeError::TooLarge)?;
    let max = round(max).unwrap_or(u64::MAX);
    if rounded > max {
        return Err(ServiceSizeError::TooLarge);
    }
    Ok(rounded)
}

/// The environment variable that turns development authentication on.
pub const DEV_AUTH_ENV: &str = "COMMY_ALLOW_DEV_AUTH";

impl Server {
    /// A server with development authentication on only if `COMMY_ALLOW_DEV_AUTH=1`.
    pub fn new() -> Self {
        let dev_auth = std::env::var(DEV_AUTH_ENV).is_ok_and(|v| v == "1");
        if dev_auth {
            eprintln!(
                "!!! {DEV_AUTH_ENV}=1: DEVELOPMENT AUTHENTICATION IS ON. Any client can \
                 authenticate to any tenant as admin. Do not use this on a reachable server. !!!"
            );
        }
        Self::with_dev_auth(dev_auth)
    }

    /// A server with development authentication set explicitly (used by tests).
    pub fn with_dev_auth(dev_auth: bool) -> Self {
        Server {
            tenants: std::collections::HashMap::new(),
            servers: vec![],
            clients: vec![],
            service_registry: std::collections::HashMap::new(),
            dev_auth,
            next_file_id: 1,
            forwarder: server::forwarding::Registry::new(),
            rotations: server::rotation::Rotations::new(),
            grants: Box::new(server::grants::RoleSystemGrants::new()),
            retired_files: std::collections::HashSet::new(),
            prepared: std::collections::HashMap::new(),
            fill_timeout: std::env::var("COMMY_FILL_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .map(std::time::Duration::from_millis)
                .unwrap_or(std::time::Duration::from_secs(30)),
            drain_timeout: std::env::var("COMMY_DRAIN_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .map(std::time::Duration::from_millis)
                .unwrap_or(std::time::Duration::from_secs(5)),
            rotation_min_interval: server::rotation::DEFAULT_MIN_INTERVAL,
            audit_log: std::sync::Mutex::new(Vec::new()),
            audit_failures: std::sync::atomic::AtomicU64::new(0),
            persistence: None,
            verifier: None,
            grant_lock: std::sync::Mutex::new(()),
            max_service_size: std::env::var(MAX_SERVICE_SIZE_ENV)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_SERVICE_SIZE),
            audit_max_events: std::env::var("COMMY_AUDIT_MAX_EVENTS")
                .ok()
                .and_then(|v| v.parse().ok()),
        }
    }

    /// Apply the limits and timeouts from `settings` (environment first, then the configuration
    /// file; see [`config`]). Call before [`Server::enable_persistence`].
    pub fn configure(&mut self, settings: &config::Settings) {
        if let Some(ms) = settings.get_parsed::<u64>("COMMY_FILL_TIMEOUT_MS") {
            self.fill_timeout = std::time::Duration::from_millis(ms);
        }
        if let Some(ms) = settings.get_parsed::<u64>("COMMY_DRAIN_TIMEOUT_MS") {
            self.drain_timeout = std::time::Duration::from_millis(ms);
        }
        if let Some(max) = settings.get_parsed::<u64>(MAX_SERVICE_SIZE_ENV) {
            self.max_service_size = max;
        }
        if let Some(keep) = settings.get_parsed::<u64>("COMMY_AUDIT_MAX_EVENTS") {
            self.audit_max_events = Some(keep);
        }
    }

    /// Open (or close) a service to local mapping. Returns false for an unknown service.
    /// (An admin message for this arrives with the service-scoped permissions, C10.)
    pub fn set_trusted_local(&mut self, service_id: &str, trusted: bool) -> bool {
        match self.service_registry.get_mut(service_id) {
            Some(record) => {
                record.trusted_local = trusted;
                true
            }
            None => false,
        }
    }

    /// As [`Server::set_trusted_local`], recording the change first and refusing it if it cannot be
    /// recorded (a close that was not recorded would reopen the service after a restart).
    pub fn set_trusted_local_recorded(&mut self, service_id: &str, trusted: bool) -> Result<bool, persist::PersistError> {
        let Some(mut row) = self.row_of(service_id) else {
            return Ok(false);
        };
        row.trusted_local = trusted;
        self.persist_row(service_id, &row)?;
        Ok(self.set_trusted_local(service_id, trusted))
    }

    /// Verify credentials with `verifier` (see [`server::verify`]).
    pub fn with_verifier(mut self, verifier: std::sync::Arc<dyn server::verify::CredentialVerifier>) -> Self {
        self.verifier = Some(verifier);
        self
    }

    /// Keep the service registry and the grants on disk under `dir` (created private), and load
    /// what is already there. Call before serving.
    pub fn enable_persistence(&mut self, dir: &std::path::Path) -> Result<StateReport, persist::PersistError> {
        self.use_persistence(persist::Persistence::open(dir)?)
    }

    /// As [`Server::enable_persistence`] on a store you opened (tests use memory and fault fakes).
    pub fn use_persistence(&mut self, p: persist::Persistence) -> Result<StateReport, persist::PersistError> {
        let mut report = StateReport::default();
        // Grants first: they do not depend on anything else.
        for op in p.load_grant_ops()? {
            let specs: Result<Vec<_>, _> = op
                .permissions
                .iter()
                .map(|n| server::grants::parse_permission(n))
                .collect();
            let applied = specs.and_then(|specs| match op.kind {
                persist::OpKind::Grant => self.grants.grant(&op.tenant, &op.client, op.service.as_deref(), &specs),
                persist::OpKind::Revoke => self.grants.revoke(&op.tenant, &op.client, op.service.as_deref(), &specs),
            });
            match applied {
                Ok(()) => report.grant_ops_replayed += 1,
                Err(e) => {
                    report.grant_ops_skipped += 1;
                    self.audit(&format!("state: skipped a stored grant operation: {e}"));
                }
            }
        }
        for (id, row) in p.load_services()? {
            let Ok(meta) = std::fs::metadata(&row.path) else {
                report.services_missing_file += 1;
                self.audit(&format!("state: service {id} skipped, file {} is gone", row.path));
                continue;
            };
            let size = meta.len() as usize;
            let tenant = self.get_tenant(&row.tenant);
            if let Err(e) = tenant.register_service(&row.name, &row.path, size) {
                report.services_missing_file += 1;
                self.audit(&format!("state: service {id} skipped, cannot map {}: {e}", row.path));
                continue;
            }
            self.next_file_id = self.next_file_id.max(row.file_id.wrapping_add(1).max(1));
            self.service_registry.insert(
                id,
                ServiceRecord {
                    tenant_name: row.tenant,
                    service_name: row.name,
                    file_path: row.path,
                    file_id: row.file_id,
                    trusted_local: row.trusted_local,
                    seq: 0,
                    generation: row.generation,
                },
            );
            report.services_loaded += 1;
        }
        if let Some(keep) = self.audit_max_events {
            p.set_audit_limit(keep);
        }
        self.persistence = Some(p);
        self.audit(&format!(
            "server STARTED state: {} service(s) loaded, {} skipped (file missing), {} grant change(s) replayed, {} skipped",
            report.services_loaded,
            report.services_missing_file,
            report.grant_ops_replayed,
            report.grant_ops_skipped
        ));
        Ok(report)
    }

    /// Record a stored row for a service (no-op without persistence).
    pub fn persist_row(&self, id: &str, row: &persist::ServiceRow) -> Result<(), persist::PersistError> {
        match &self.persistence {
            Some(p) => p.put_service(id, row),
            None => Ok(()),
        }
    }

    /// Forget the stored row of a service (no-op without persistence).
    pub fn forget_row(&self, id: &str) -> Result<(), persist::PersistError> {
        match (&self.persistence, self.service_registry.get(id)) {
            (Some(p), Some(r)) => p.delete_service(id, &r.tenant_name, &r.service_name),
            _ => Ok(()),
        }
    }

    /// The stored form of a registered service.
    pub fn row_of(&self, id: &str) -> Option<persist::ServiceRow> {
        self.service_registry.get(id).map(|r| persist::ServiceRow {
            tenant: r.tenant_name.clone(),
            name: r.service_name.clone(),
            path: r.file_path.clone(),
            file_id: r.file_id,
            generation: r.generation,
            trusted_local: r.trusted_local,
        })
    }

    /// Give `client` permissions. With persistence the change is recorded first and refused if it
    /// cannot be, so a restart never loses it.
    pub fn grant(
        &self,
        tenant: &str,
        client: &str,
        service: Option<&str>,
        specs: &[server::grants::Spec],
    ) -> Result<(), server::grants::GrantError> {
        self.change_grants(persist::OpKind::Grant, tenant, client, service, specs)
    }

    /// Take permissions away; same recording rule as [`Server::grant`]. A revoke that cannot be
    /// recorded is NOT applied: otherwise a restart would give the access back.
    pub fn revoke(
        &self,
        tenant: &str,
        client: &str,
        service: Option<&str>,
        specs: &[server::grants::Spec],
    ) -> Result<(), server::grants::GrantError> {
        self.change_grants(persist::OpKind::Revoke, tenant, client, service, specs)
    }

    fn change_grants(
        &self,
        kind: persist::OpKind,
        tenant: &str,
        client: &str,
        service: Option<&str>,
        specs: &[server::grants::Spec],
    ) -> Result<(), server::grants::GrantError> {
        use server::grants::{GrantError, Spec};
        // Refuse what the store would refuse *before* recording it.
        if service.is_some() && specs.iter().any(|s| matches!(s, Spec::TenantAdmin)) {
            return Err(GrantError::WrongScope("tenant:admin".to_string()));
        }
        let _order = self.grant_lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = &self.persistence {
            p.append_grant_op(&persist::GrantOp {
                kind,
                tenant: tenant.to_string(),
                client: client.to_string(),
                service: service.map(str::to_string),
                permissions: specs.iter().map(|s| s.wire_name().to_string()).collect(),
            })
            .map_err(|e| GrantError::Backend(e.to_string()))?;
        }
        match kind {
            persist::OpKind::Grant => self.grants.grant(tenant, client, service, specs),
            persist::OpKind::Revoke => self.grants.revoke(tenant, client, service, specs),
        }
    }

    /// Set how long a resize waits for SDKs to drain.
    pub fn with_drain_timeout(mut self, t: std::time::Duration) -> Self {
        self.drain_timeout = t;
        self
    }

    /// Set how long a requester has to fill a prepared file.
    pub fn with_fill_timeout(mut self, t: std::time::Duration) -> Self {
        self.fill_timeout = t;
        self
    }

    /// Set the least time between two rotations of one service.
    pub fn with_rotation_min_interval(mut self, t: std::time::Duration) -> Self {
        self.rotation_min_interval = t;
        self
    }

    /// Record a security-relevant event: printed to stderr, kept in memory (the last 10,000), and,
    /// with persistence, appended to the durable audit log. A failed write to the durable log is
    /// counted ([`Server::audit_failures`]) and printed; it never blocks the operation it describes
    /// (the changes that matter are recorded fail-closed by the state store itself).
    pub fn audit(&self, line: &str) {
        eprintln!("[audit] {line}");
        {
            let mut log = self.audit_log.lock().unwrap_or_else(|e| e.into_inner());
            if log.len() >= 10_000 {
                log.remove(0);
            }
            log.push(line.to_string());
        }
        if let Some(p) = &self.persistence {
            if let Err(e) = p.append_audit(line) {
                self.audit_failures
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                eprintln!("[audit] NOT RECORDED in the durable log: {e}");
            }
        }
    }

    /// Keep only the newest `keep` audit events in the durable log (no-op without persistence).
    /// `COMMY_AUDIT_MAX_EVENTS` sets it at start-up.
    pub fn set_audit_limit(&self, keep: u64) {
        if let Some(p) = &self.persistence {
            p.set_audit_limit(keep);
        }
    }

    /// How many audit events could not be written to the durable log since start-up.
    pub fn audit_failures(&self) -> u64 {
        self.audit_failures
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Stored audit events after sequence `after` (all if `None`), oldest first, at most `limit`.
    /// Empty without persistence.
    pub fn audit_history(
        &self,
        after: Option<u64>,
        limit: usize,
    ) -> Result<Vec<(u64, persist::AuditEvent)>, persist::PersistError> {
        match &self.persistence {
            Some(p) => p.load_audit(after, limit),
            None => Ok(Vec::new()),
        }
    }

    /// The recorded audit events, oldest first.
    pub fn audit_events(&self) -> Vec<String> {
        self.audit_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Set the largest service file a client may ask for (bytes).
    pub fn with_max_service_size(mut self, max: u64) -> Self {
        self.max_service_size = max;
        self
    }

    pub fn get_tenant(&mut self, tenant_name: &str) -> &mut Tenant {
        let tenant_name_owned = tenant_name.to_string();
        self.tenants
            .entry(tenant_name_owned.clone())
            .or_insert_with(|| {
                // Determine storage backend based on DATABASE_URL environment variable
                let storage_backend = if let Ok(db_url) = std::env::var("DATABASE_URL") {
                    eprintln!("[TENANTS] DATABASE_URL found: {}", db_url);
                    // Parse the database URL to determine type and extract connection details
                    if db_url.starts_with("postgresql://") {
                        eprintln!(
                            "[TENANTS] Using PostgreSQL backend for {}",
                            tenant_name_owned
                        );
                        auth::tenant_context::StorageBackend::PostgreSQL {
                            url: db_url.clone(),
                            max_connections: 100,
                        }
                    } else if db_url.starts_with("mysql://") {
                        eprintln!("[TENANTS] Using MySQL backend for {}", tenant_name_owned);
                        auth::tenant_context::StorageBackend::MySQL {
                            url: db_url.clone(),
                            max_connections: 100,
                        }
                    } else {
                        // Default to memory if URL doesn't match known databases
                        eprintln!(
                            "[TENANTS] Unknown database type, falling back to Memory: {}",
                            db_url
                        );
                        auth::tenant_context::StorageBackend::Memory
                    }
                } else {
                    // No DATABASE_URL set, use memory storage
                    eprintln!("[TENANTS] DATABASE_URL not set, using Memory storage");
                    auth::tenant_context::StorageBackend::Memory
                };

                // Create default auth context for this tenant
                let config = auth::tenant_context::TenantAuthConfig {
                    tenant_id: tenant_name_owned.clone(),
                    mode: auth::tenant_context::AuthenticationMode::ServerManaged,
                    auth_methods: vec!["jwt".to_string(), "api_key".to_string()],
                    callback_endpoint: None,
                    callback_timeout: std::time::Duration::from_secs(5),
                    require_mfa: false,
                    token_lifetime_secs: 3600,
                    max_failed_logins: 5,
                    lockout_duration_secs: 300,
                    storage_backend,
                };
                let auth_context = TenantAuthContext::new(config);
                Tenant::new(tenant_name_owned, auth_context)
            })
    }
}

pub struct Client {
    // Client-specific fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use tempfile::NamedTempFile;

    // ─── Service ───────────────────────────────────────────────────────────────

    #[test]
    fn test_service_new_opens_existing_file() {
        // Service::new() must be able to open an existing writable file and
        // memory-map it for read+write (not read-only).
        let tmp = NamedTempFile::new().unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp.path())
            .unwrap();
        file.set_len(65536).unwrap();
        drop(file);
        // Should not panic – requires write access for MmapMut
        let _svc = Service::new(tmp.path().to_str().unwrap());
    }

    #[test]
    fn test_tenant_id() {
        unsafe { std::env::set_var("ENVIRONMENT", "development"); }
        let config = auth::tenant_context::TenantAuthConfig {
            tenant_id: "my_tenant".to_string(),
            mode: auth::tenant_context::AuthenticationMode::ServerManaged,
            auth_methods: vec![],
            callback_endpoint: None,
            callback_timeout: std::time::Duration::from_secs(5),
            require_mfa: false,
            token_lifetime_secs: 3600,
            max_failed_logins: 5,
            lockout_duration_secs: 300,
            storage_backend: auth::tenant_context::StorageBackend::Memory,
        };
        let ctx = TenantAuthContext::new(config);
        let tenant = Tenant::new("my_tenant".to_string(), ctx);
        assert_eq!(tenant.id(), "my_tenant");
    }

    #[test]
    fn test_tenant_auth_context_ref() {
        unsafe { std::env::set_var("ENVIRONMENT", "development"); }
        let config = auth::tenant_context::TenantAuthConfig {
            tenant_id: "t1".to_string(),
            mode: auth::tenant_context::AuthenticationMode::ServerManaged,
            auth_methods: vec![],
            callback_endpoint: None,
            callback_timeout: std::time::Duration::from_secs(5),
            require_mfa: false,
            token_lifetime_secs: 3600,
            max_failed_logins: 5,
            lockout_duration_secs: 300,
            storage_backend: auth::tenant_context::StorageBackend::Memory,
        };
        let ctx = TenantAuthContext::new(config);
        let tenant = Tenant::new("t1".to_string(), ctx);
        // Just verify the Arc is accessible
        let _ctx_ref = tenant.auth_context();
    }

    #[test]
    fn test_tenant_register_and_get_service() {
        unsafe { std::env::set_var("ENVIRONMENT", "development"); }
        let tmp = NamedTempFile::new().unwrap();
        {
            let f = OpenOptions::new().read(true).write(true).open(tmp.path()).unwrap();
            f.set_len(65536).unwrap();
        }
        let config = auth::tenant_context::TenantAuthConfig {
            tenant_id: "t2".to_string(),
            mode: auth::tenant_context::AuthenticationMode::ServerManaged,
            auth_methods: vec![],
            callback_endpoint: None,
            callback_timeout: std::time::Duration::from_secs(5),
            require_mfa: false,
            token_lifetime_secs: 3600,
            max_failed_logins: 5,
            lockout_duration_secs: 300,
            storage_backend: auth::tenant_context::StorageBackend::Memory,
        };
        let ctx = TenantAuthContext::new(config);
        let mut tenant = Tenant::new("t2".to_string(), ctx);
        tenant
            .register_service("svc", tmp.path().to_str().unwrap(), 65536)
            .unwrap();
        assert!(tenant.get_service_by_name("svc").is_some());
        assert!(tenant.get_service_mut_by_name("svc").is_some());
        assert!(tenant.get_service_by_name("nope").is_none());
    }

    #[test]
    fn test_server_get_tenant_creates_and_caches() {
        unsafe { std::env::set_var("ENVIRONMENT", "development"); }
        let mut server = Server::new();

        // First call creates the tenant
        {
            let t = server.get_tenant("org_a");
            assert_eq!(t.id(), "org_a");
        }

        // Second call returns the cached tenant
        {
            let t = server.get_tenant("org_a");
            assert_eq!(t.id(), "org_a");
        }

        // Only one tenant stored
        assert_eq!(server.tenants.len(), 1);

        // Different name creates a second tenant
        {
            let t2 = server.get_tenant("org_b");
            assert_eq!(t2.id(), "org_b");
        }
        assert_eq!(server.tenants.len(), 2);
    }

    #[test]
    fn a_service_file_is_bare_zero_filled_bytes() {
        // Plan C9: nothing is written into a service file; it is the requested number of zero
        // bytes from offset 0 and stays that way until a client writes.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc.mem");
        let svc = Service::open_or_create(path.to_str().unwrap(), 8192).unwrap();
        assert_eq!(svc.len(), 8192);
        assert!(svc.as_slice().iter().all(|b| *b == 0), "no header, no bookkeeping");
        drop(svc);
        let on_disk = std::fs::read(&path).unwrap();
        assert_eq!(on_disk.len(), 8192);
        assert!(on_disk.iter().all(|b| *b == 0));
    }

    #[test]
    fn opening_an_existing_file_keeps_its_bytes_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc.mem");
        std::fs::write(&path, [1u8, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let svc = Service::open_or_create(path.to_str().unwrap(), 4096).unwrap();
        assert_eq!(svc.len(), 8, "an existing file is not resized");
        assert_eq!(svc.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn writes_through_the_mapping_reach_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("svc.mem");
        let mut svc = Service::open_or_create(path.to_str().unwrap(), 4096).unwrap();
        svc.as_mut_slice()[100..104].copy_from_slice(&[9, 8, 7, 6]);
        svc.flush().unwrap();
        assert_eq!(&std::fs::read(&path).unwrap()[100..104], &[9, 8, 7, 6]);
    }
}
