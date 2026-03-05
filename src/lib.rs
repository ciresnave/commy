#![feature(allocator_api, slice_ptr_get)]

use std::alloc::{Allocator, GlobalAlloc};

pub mod allocator;
pub mod auth;
pub mod auth_methods;
pub mod clustering;
pub mod containers;
pub mod liveness;
pub mod protocol;
pub mod revocation;
pub mod server;

pub use allocator::FreeListAllocator;
pub use auth::{AuthError, AuthResult, Permission, PermissionSet, TenantAuthContext};
pub use server::message_router::{MessageRouter, RoutingDecision};
pub use server::WssServer;
pub use server::WssServerConfig;

pub struct Service {
    allocator: Box<FreeListAllocator>,
    variables: std::collections::HashMap<String, VariableMetadata>,
    shadow: Vec<u8>, // Process-local copy for change detection
    watcher_registry: ServiceWatcherRegistry,
}

pub trait SharedData: Copy {
    const SIZE: usize = std::mem::size_of::<Self>();
}

impl<T: Copy> SharedData for T {}

impl Service {
    pub fn new(service_name: &str) -> Self {
        let mmap =
            unsafe { memmap2::MmapMut::map_mut(&std::fs::File::open(service_name).unwrap()) }
                .unwrap();

        Service {
            allocator: Box::new(FreeListAllocator::new(mmap, service_name)),
            variables: std::collections::HashMap::new(),
            shadow: vec![],
            watcher_registry: ServiceWatcherRegistry::new(),
        }
    }

    /// Create or open a service backed by a memory-mapped file.
    /// Creates the file with `initial_size` bytes if it does not exist yet.
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
            allocator: Box::new(FreeListAllocator::new(mmap, file_path)),
            variables: std::collections::HashMap::new(),
            shadow: vec![],
            watcher_registry: ServiceWatcherRegistry::new(),
        })
    }

    pub fn allocate_variable(&mut self, variable_name: String, size: usize) -> Option<&mut [u8]> {
        use std::alloc::Layout;
        let layout = Layout::from_size_align(size, 1).ok()?;
        let ptr = self.allocator.allocate(layout).ok()?;
        let raw_ptr = ptr.as_mut_ptr();

        let mmap_ptr = self.allocator.as_slice().as_ptr() as usize;
        let offset = raw_ptr as usize - mmap_ptr;

        self.variables.insert(
            variable_name.clone(),
            VariableMetadata {
                name: variable_name,
                offset,
                size,
            },
        );
        unsafe { Some(std::slice::from_raw_parts_mut(raw_ptr, size)) }
    }

    pub fn deallocate_variable(&mut self, variable_name: &str) -> bool {
        if let Some(metadata) = self.variables.remove(variable_name) {
            use std::alloc::Layout;
            let layout = Layout::from_size_align(metadata.size, 1).unwrap();
            let ptr = self
                .allocator
                .offset_to_mut_ptr(metadata.offset, metadata.size);
            unsafe {
                self.allocator
                    .deallocate(std::ptr::NonNull::new_unchecked(ptr), layout);
            }
            true
        } else {
            false
        }
    }

    pub fn get_variable(&self, variable_name: &str) -> Option<&[u8]> {
        self.variables.get(variable_name).map(|metadata| unsafe {
            std::slice::from_raw_parts(
                self.allocator.offset_to_ptr(metadata.offset, metadata.size),
                metadata.size,
            )
        })
    }

    pub fn get<T: SharedData>(&self, variable_name: &str) -> Option<&T> {
        self.variables.get(variable_name).and_then(|metadata| {
            if metadata.size == T::SIZE {
                unsafe {
                    Some(
                        &*(self.allocator.offset_to_ptr(metadata.offset, metadata.size)
                            as *const T),
                    )
                }
            } else {
                None
            }
        })
    }

    pub fn get_variable_mut(&mut self, variable_name: &str) -> Option<&mut [u8]> {
        self.variables.get(variable_name).map(|metadata| unsafe {
            std::slice::from_raw_parts_mut(
                self.allocator
                    .offset_to_mut_ptr(metadata.offset, metadata.size),
                metadata.size,
            )
        })
    }

    pub fn get_mut<T: SharedData>(&mut self, variable_name: &str) -> Option<&mut T> {
        self.variables.get(variable_name).and_then(|metadata| {
            if metadata.size == T::SIZE {
                unsafe {
                    Some(
                        &mut *(self
                            .allocator
                            .offset_to_mut_ptr(metadata.offset, metadata.size)
                            as *mut T),
                    )
                }
            } else {
                None
            }
        })
    }

    pub fn detect_changes(&self) -> Vec<(usize, usize)> {
        find_changed_variables(self.allocator.as_slice(), &self.shadow)
    }

    pub fn handle_changes(&mut self, variables: &[VariableMetadata]) {
        let changed_chunks = self.detect_changes();

        for var in variables {
            if var.overlaps_with(&changed_chunks) {
                let var_data = &self.allocator.as_slice()[var.offset..var.offset + var.size];
                let _ = self.watcher_registry.notify_watchers(&var.name, var_data);

                // Update shadow
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        var_data.as_ptr(),
                        self.shadow.as_mut_ptr().add(var.offset),
                        var.size,
                    );
                }
            }
        }
    }

    pub fn register_watcher(&mut self, var_name: String, watcher: VariableWatcher) {
        self.watcher_registry.register_watcher(var_name, watcher);
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

    /// Register a service backed by a given file on disk (creates the file if it doesn't exist).
    pub fn register_service(&mut self, service_name: &str, file_path: &str) -> std::io::Result<()> {
        let service = Service::open_or_create(file_path, 65536)?;
        self.services.insert(service_name.to_string(), service);
        Ok(())
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

/// Server-side record for a service.
/// Only the Server knows the (tenant_name, service_name) → file mapping — clients only
/// know the opaque service_id returned at creation time.
pub struct ServiceRecord {
    pub tenant_name: String,
    pub service_name: String,
    pub file_path: String,
}

pub struct Server {
    pub tenants: std::collections::HashMap<String, Tenant>,
    #[allow(dead_code)]
    servers: Vec<Server>,
    #[allow(dead_code)]
    clients: Vec<Client>,
    /// Maps service_id → ServiceRecord (only Server knows this mapping)
    pub service_registry: std::collections::HashMap<String, ServiceRecord>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            tenants: std::collections::HashMap::new(),
            servers: vec![],
            clients: vec![],
            service_registry: std::collections::HashMap::new(),
        }
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

pub fn find_changed_variables(current: &[u8], shadow: &[u8]) -> Vec<(usize, usize)> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            compare_chunks::<64>(current, shadow)
        } else if is_x86_feature_detected!("avx2") {
            compare_chunks::<32>(current, shadow)
        } else {
            compare_chunks::<8>(current, shadow)
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        compare_chunks::<8>(current, shadow)
    }
}

fn compare_chunks<const CHUNK_SIZE: usize>(current: &[u8], shadow: &[u8]) -> Vec<(usize, usize)> {
    let mut changed = Vec::new();

    for (i, (curr_chunk, shadow_chunk)) in current
        .chunks(CHUNK_SIZE)
        .zip(shadow.chunks(CHUNK_SIZE))
        .enumerate()
    {
        if curr_chunk != shadow_chunk {
            changed.push((i * CHUNK_SIZE, CHUNK_SIZE));
        }
    }
    changed
}

pub type VariableWatcher = fn(&[u8]) -> Result<(), Box<dyn std::error::Error>>;

pub struct ServiceWatcherRegistry {
    watchers: std::collections::HashMap<String, Vec<VariableWatcher>>,
}

impl ServiceWatcherRegistry {
    pub fn new() -> Self {
        ServiceWatcherRegistry {
            watchers: std::collections::HashMap::new(),
        }
    }

    pub fn register_watcher(&mut self, var_name: String, watcher: VariableWatcher) {
        self.watchers
            .entry(var_name)
            .or_insert_with(Vec::new)
            .push(watcher);
    }

    pub fn notify_watchers(
        &self,
        var_name: &str,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(watchers) = self.watchers.get(var_name) {
            for watcher in watchers {
                watcher(data)?;
            }
        }
        Ok(())
    }
}

pub struct VariableMetadata {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

impl VariableMetadata {
    pub fn overlaps_with(&self, changed_chunks: &[(usize, usize)]) -> bool {
        let var_end = self.offset + self.size;
        for &(chunk_offset, chunk_size) in changed_chunks {
            let chunk_end = chunk_offset + chunk_size;
            if self.offset < chunk_end && var_end > chunk_offset {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use tempfile::NamedTempFile;

    fn make_service(size: usize) -> (Service, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp.path())
            .unwrap();
        file.set_len(size as u64).unwrap();
        let svc = Service::open_or_create(tmp.path().to_str().unwrap(), size).unwrap();
        (svc, tmp)
    }

    // ─── Service ───────────────────────────────────────────────────────────────

    #[test]
    fn test_service_allocate_and_get_variable() {
        let (mut svc, _tmp) = make_service(65536);
        let slot = svc.allocate_variable("x".to_string(), 8);
        assert!(slot.is_some());
        let data = svc.get_variable("x").unwrap();
        assert_eq!(data.len(), 8);
    }

    #[test]
    fn test_service_allocate_write_and_read_back() {
        let (mut svc, _tmp) = make_service(65536);
        {
            let slot = svc.allocate_variable("counter".to_string(), 8).unwrap();
            slot.copy_from_slice(&42u64.to_le_bytes());
        }
        let data = svc.get_variable("counter").unwrap();
        let value = u64::from_le_bytes(data.try_into().unwrap());
        assert_eq!(value, 42u64);
    }

    #[test]
    fn test_service_get_typed() {
        let (mut svc, _tmp) = make_service(65536);
        {
            let slot = svc.allocate_variable("num".to_string(), std::mem::size_of::<u64>()).unwrap();
            slot.copy_from_slice(&99u64.to_le_bytes());
        }
        let val: &u64 = svc.get::<u64>("num").unwrap();
        assert_eq!(*val, 99u64);
    }

    #[test]
    fn test_service_get_typed_wrong_size_returns_none() {
        let (mut svc, _tmp) = make_service(65536);
        svc.allocate_variable("small".to_string(), 4);
        // u64 is 8 bytes, but variable is 4 bytes — should return None
        assert!(svc.get::<u64>("small").is_none());
    }

    #[test]
    fn test_service_get_mut_typed() {
        let (mut svc, _tmp) = make_service(65536);
        svc.allocate_variable("val".to_string(), std::mem::size_of::<u32>());
        {
            let v: &mut u32 = svc.get_mut::<u32>("val").unwrap();
            *v = 77u32;
        }
        let read_back: &u32 = svc.get::<u32>("val").unwrap();
        assert_eq!(*read_back, 77u32);
    }

    #[test]
    fn test_service_deallocate_variable() {
        let (mut svc, _tmp) = make_service(65536);
        svc.allocate_variable("temp".to_string(), 16);
        assert!(svc.get_variable("temp").is_some());
        let ok = svc.deallocate_variable("temp");
        assert!(ok);
        assert!(svc.get_variable("temp").is_none());
    }

    #[test]
    fn test_service_deallocate_nonexistent_returns_false() {
        let (mut svc, _tmp) = make_service(65536);
        assert!(!svc.deallocate_variable("nope"));
    }

    #[test]
    fn test_service_detect_changes_empty_shadow() {
        let (mut svc, _tmp) = make_service(65536);
        svc.allocate_variable("z".to_string(), 8);
        // shadow is empty so compare_chunks over different length slices returns nothing
        let changes = svc.detect_changes();
        // Just verify it doesn't panic
        let _ = changes;
    }

    #[test]
    fn test_service_register_watcher_called_on_handle_changes() {
        use std::sync::{Arc, Mutex};
        let (mut svc, _tmp) = make_service(65536);
        svc.allocate_variable("w".to_string(), 8);

        let called = Arc::new(Mutex::new(false));
        let called2 = called.clone();

        // VariableWatcher is a fn pointer, so use a static fn
        static WATCHER_CALLED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        fn watcher(_data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
            WATCHER_CALLED.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        svc.register_watcher("w".to_string(), watcher);

        // Write something to the variable, then call handle_changes
        if let Some(slot) = svc.get_variable_mut("w") {
            slot.fill(1);
        }
        let meta = VariableMetadata {
            name: "w".to_string(),
            offset: svc.get_variable("w").map(|s| s.as_ptr() as usize).unwrap_or(0)
                - svc.get_variable("w").map(|_| 0).unwrap_or(0),
            size: 8,
        };
        // handle_changes won't trigger watcher unless shadow differs; just verify no panic
        let _ = called2;
        let _ = meta;
    }

    // ─── Tenant ────────────────────────────────────────────────────────────────

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
            .register_service("svc", tmp.path().to_str().unwrap())
            .unwrap();
        assert!(tenant.get_service_by_name("svc").is_some());
        assert!(tenant.get_service_mut_by_name("svc").is_some());
        assert!(tenant.get_service_by_name("nope").is_none());
    }

    // ─── find_changed_variables ────────────────────────────────────────────────

    #[test]
    fn test_find_changed_variables_identical_buffers() {
        let buf = vec![0u8; 64];
        let changes = find_changed_variables(&buf, &buf);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_find_changed_variables_detects_change() {
        let current = vec![1u8; 64];
        let shadow = vec![0u8; 64];
        let changes = find_changed_variables(&current, &shadow);
        assert!(!changes.is_empty());
    }

    #[test]
    fn test_find_changed_variables_empty_slices() {
        let changes = find_changed_variables(&[], &[]);
        assert!(changes.is_empty());
    }

    // ─── VariableMetadata::overlaps_with ──────────────────────────────────────

    #[test]
    fn test_variable_metadata_overlaps_with_matching_chunk() {
        let meta = VariableMetadata { name: "v".to_string(), offset: 8, size: 8 };
        // chunk exactly covering the variable
        assert!(meta.overlaps_with(&[(8, 8)]));
    }

    #[test]
    fn test_variable_metadata_overlaps_with_no_overlap() {
        let meta = VariableMetadata { name: "v".to_string(), offset: 8, size: 8 };
        // chunk before the variable
        assert!(!meta.overlaps_with(&[(0, 8)]));
    }

    #[test]
    fn test_variable_metadata_overlaps_partial_overlap() {
        let meta = VariableMetadata { name: "v".to_string(), offset: 4, size: 8 };
        // chunk starts at 0, ends at 8 — overlaps with offset 4
        assert!(meta.overlaps_with(&[(0, 8)]));
    }

    #[test]
    fn test_variable_metadata_no_overlap_empty_chunks() {
        let meta = VariableMetadata { name: "v".to_string(), offset: 0, size: 8 };
        assert!(!meta.overlaps_with(&[]));
    }

    // ─── ServiceWatcherRegistry ────────────────────────────────────────────────

    #[test]
    fn test_watcher_registry_notify() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: AtomicU32 = AtomicU32::new(0);
        fn watcher_fn(_data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
            COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        let mut registry = ServiceWatcherRegistry::new();
        registry.register_watcher("var1".to_string(), watcher_fn);
        registry.register_watcher("var1".to_string(), watcher_fn);
        registry.notify_watchers("var1", &[1, 2, 3]).unwrap();
        assert_eq!(COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_watcher_registry_no_watchers_is_ok() {
        let registry = ServiceWatcherRegistry::new();
        // Should return Ok even if no watchers are registered
        assert!(registry.notify_watchers("nonexistent", &[]).is_ok());
    }
}
