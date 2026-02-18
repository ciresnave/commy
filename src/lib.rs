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
}

pub struct Server {
    pub tenants: std::collections::HashMap<String, Tenant>,
    #[allow(dead_code)]
    servers: Vec<Server>,
    #[allow(dead_code)]
    clients: Vec<Client>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            tenants: std::collections::HashMap::new(),
            servers: vec![],
            clients: vec![],
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
