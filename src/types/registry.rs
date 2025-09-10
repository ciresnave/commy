use std::sync::Arc;

use crate::errors::CommyError;
use bitflags::bitflags;
use dashmap::DashMap;
use thiserror::Error;

bitflags! {
    /// Supported serialization formats bitflags.
    pub struct Formats: u32 {
        const JSON = 0b0001;
        const BINARY = 0b0010;
        const MESSAGEPACK = 0b0100;
        const RKYV = 0b1000;
        const CAPNP = 0b1_0000;
    }
}

/// Error returned by registry operations
#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("type already registered")]
    AlreadyRegistered,
    #[error("not found")]
    NotFound,
}

/// Thin serialized writer signature used for compiled-in types
pub type WriterFn = fn(&dyn std::any::Any, &mut [u8]) -> Result<usize, CommyError>;

/// A TypeEntry describes a registered type and available serializers
pub struct TypeEntry {
    pub type_name: String,
    pub schema_hash: u64,
    pub formats: Formats,
    pub writer: Option<WriterFn>,
}

impl TypeEntry {
    pub fn key(&self) -> (String, u64) {
        (self.type_name.clone(), self.schema_hash)
    }
}

/// Global registry type
pub struct Registry {
    inner: DashMap<(String, u64), Arc<TypeEntry>>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            inner: DashMap::new(),
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new()
    }
}

impl Registry {
    /// Register a compiled-in type. Returns error if the same key already exists.
    pub fn register_compiled(&self, entry: TypeEntry) -> Result<(), RegistryError> {
        let key = entry.key();
        if self.inner.contains_key(&key) {
            return Err(RegistryError::AlreadyRegistered);
        }
        self.inner.insert(key, Arc::new(entry));
        Ok(())
    }

    /// Register a plugin-provided TypeEntry (idempotent)
    pub fn register_plugin(&self, entry: TypeEntry) {
        let key = entry.key();
        self.inner.insert(key, Arc::new(entry));
    }

    pub fn lookup(&self, type_name: &str, schema_hash: Option<u64>) -> Option<Arc<TypeEntry>> {
        if let Some(hash) = schema_hash {
            self.inner
                .get(&(type_name.to_string(), hash))
                .map(|r| r.clone())
        } else {
            // Find by name with any hash (first match)
            self.inner.iter().find_map(|kv| {
                if kv.key().0 == type_name {
                    Some(kv.value().clone())
                } else {
                    None
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_basic() {
        let reg = Registry::new();
        let entry = TypeEntry {
            type_name: "Test".to_string(),
            schema_hash: 12345,
            formats: Formats::JSON,
            writer: None,
        };

        assert!(reg.register_compiled(entry).is_ok());
        let found = reg.lookup("Test", Some(12345)).expect("should find");
        assert_eq!(found.type_name, "Test");
    }
}
