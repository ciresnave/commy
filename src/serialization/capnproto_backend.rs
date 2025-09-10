use crate::serialization::{SerializationBackend, SerializationError};

/// Cap'n Proto serialization backend - zero-copy polyglot serialization
///
/// This backend provides true zero-copy serialization that works across multiple
/// programming languages, making it ideal for Commy's polyglot service mesh architecture.
///
/// Benefits over other formats:
/// - Zero-copy reading in all supported languages (C++, Python, JavaScript, Go, Java, C#, Rust)
/// - Schema evolution with backward/forward compatibility
/// - Designed for RPC and shared memory scenarios
/// - Strong typing with code generation
/// - Excellent performance for complex nested data structures
#[cfg(feature = "capnproto")]
#[derive(Debug)]
pub struct CapnProtoBackend;

#[cfg(feature = "capnproto")]
impl SerializationBackend for CapnProtoBackend {
    fn name() -> &'static str {
        "Cap'n Proto"
    }

    fn serialize<T>(value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: ?Sized + serde::Serialize,
    {
        // For now, we use serde as a bridge since the trait is constrained to serde::Serialize
        // In a production implementation, we'd generate Cap'n Proto schemas and use those directly
        // This would be much more efficient than going through serde

        // TODO: Generate Cap'n Proto schemas for core Commy types
        // TODO: Implement direct Cap'n Proto serialization bypassing serde

        // Temporary bridge implementation
        serde_json::to_vec(value).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            // Use JsonSerialization for the serde bridge error but indicate Compact
            // as the intended target format for mapping context.
            let com_err = crate::errors::CommyError::JsonSerialization(e);
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::Compact));
            SerializationError::SerializationFailed(format!("{:?}", trans_err))
        })
    }

    fn deserialize<T>(data: &[u8]) -> Result<T, SerializationError>
    where
        T: serde::de::DeserializeOwned,
    {
        // TODO: Implement direct Cap'n Proto deserialization
        serde_json::from_slice(data).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            let com_err = crate::errors::CommyError::JsonSerialization(e);
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::Compact));
            SerializationError::DeserializationFailed(format!("{:?}", trans_err))
        })
    }
}

/// Specialized Cap'n Proto utilities for true polyglot zero-copy performance
#[cfg(feature = "capnproto")]
pub struct CapnProtoSerializer;

#[cfg(feature = "capnproto")]
impl CapnProtoSerializer {
    /// Create a Cap'n Proto message builder
    pub fn create_builder() -> capnp::message::Builder<capnp::message::HeapAllocator> {
        capnp::message::Builder::new_default()
    }

    /// Serialize a Cap'n Proto message to bytes
    pub fn serialize_message(
        builder: &capnp::message::Builder<capnp::message::HeapAllocator>,
    ) -> Result<Vec<u8>, SerializationError> {
        let words = capnp::serialize::write_message_to_words(builder);
        Ok(words)
    }

    /// Deserialize bytes into a Cap'n Proto message reader
    pub fn deserialize_message(
        data: &[u8],
    ) -> Result<capnp::message::Reader<capnp::serialize::OwnedSegments>, SerializationError> {
        capnp::serialize::read_message(data, capnp::message::ReaderOptions::new()).map_err(|e| {
            SerializationError::DeserializationFailed(format!(
                "Cap'n Proto deserialization error: {:?}",
                e
            ))
        })
    }

    /// Get schema text for generating language bindings
    /// This would contain the Cap'n Proto schema definitions for core Commy types
    pub fn get_commy_schema() -> &'static str {
        r#"
# Commy Service Mesh Schema
# This schema defines the core data types used in Commy's polyglot communication

@0x9eb32e19f86ee174;

struct ServiceInfo {
  id @0 :Text;
  name @1 :Text;
  host @2 :Text;
  port @3 :UInt16;
  tags @4 :List(Text);
  metadata @5 :List(KeyValue);
  health @6 :HealthStatus;
  lastSeen @7 :UInt64; # Unix timestamp
}

struct KeyValue {
  key @0 :Text;
  value @1 :Text;
}

enum HealthStatus {
  unknown @0;
  healthy @1;
  unhealthy @2;
  critical @3;
}

struct MeshInfo {
  nodeId @0 :Text;
  services @1 :List(ServiceInfo);
  meshStatus @2 :MeshStatus;
  statistics @3 :MeshStatistics;
}

enum MeshStatus {
  stopped @0;
  starting @1;
  running @2;
  stopping @3;
  error @4;
}

struct MeshStatistics {
  totalServices @0 :UInt32;
  healthyServices @1 :UInt32;
  totalMessages @2 :UInt64;
  totalBytes @3 :UInt64;
  uptime @4 :UInt64; # Seconds
}

# Shared memory communication structures
struct SharedMessage {
  messageId @0 :Text;
  sourceService @1 :Text;
  targetService @2 :Text;
  payload @3 :Data;
  timestamp @4 :UInt64;
  metadata @5 :List(KeyValue);
}

struct FileHeader {
  version @0 :UInt32;
  format @1 :Text; # "capnproto", "rkyv", "json", etc.
  created @2 :UInt64;
  lastModified @3 :UInt64;
  size @4 :UInt64;
  checksum @5 :UInt64;
}
"#
    }
}

#[cfg(test)]
#[cfg(feature = "capnproto")]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
        values: Vec<i32>,
    }

    #[test]
    fn test_capnproto_backend() {
        let data = TestData {
            id: 42,
            name: "capnproto_test".to_string(),
            values: vec![1, 2, 3, 4, 5],
        };

        let serialized = CapnProtoBackend::serialize(&data).expect("Serialization failed");
        let deserialized =
            CapnProtoBackend::deserialize::<TestData>(&serialized).expect("Deserialization failed");

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_capnproto_utilities() {
        // Test basic Cap'n Proto functionality
        let _builder = CapnProtoSerializer::create_builder();

        // For now, just test that we can create a builder
        // In a full implementation, we'd test actual schema operations
        assert!(!CapnProtoSerializer::get_commy_schema().is_empty());
    }
    #[test]
    fn test_schema_content() {
        let schema = CapnProtoSerializer::get_commy_schema();

        // Verify our schema contains expected structures
        assert!(schema.contains("ServiceInfo"));
        assert!(schema.contains("MeshInfo"));
        assert!(schema.contains("SharedMessage"));
        assert!(schema.contains("HealthStatus"));
    }
}
