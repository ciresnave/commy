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
        // FIXME: Temporary serde_json bridge
        //
        // This implementation exists only because the public
        // `SerializationBackend` trait currently requires `serde::Serialize`.
        // That trait constraint forces us to use serde (here via JSON) as a
        // bridge to obtain bytes. This undermines Commy's performance goals
        // (zero-copy, cross-language Cap'n Proto and rkyv integration).
        //
        // Long-term approach:
        //  - Replace the serde-bound trait with a generated trait (for
        //    example `CapnpSerialize`) produced by a proc-macro. The macro
        //    will emit per-type Cap'n Proto (and rkyv) builders/readers.
        //  - The generated trait should provide methods like:
        //      fn capnp_serialize<'a, B: capnp::private::layout::Allocator +'a>(&self, builder: &'a mut <Self as CapnpSchema>::Builder<'a>);
        //      fn capnp_deserialize<'a>(reader: <Self as CapnpSchema>::Reader<'a>) -> Result<Self, Error>;
        //  - The proc-macro would also emit the corresponding .capnp schema
        //    text and optionally write it to `OUT_DIR` so language bindings
        //    can be generated for polyglot consumers.
        //
        // Example of the desired direct implementation (pseudocode):
        //
        // let mut message = capnp::message::Builder::new_default();
        // let mut root = message.init_root::<<T as CapnpSchema>::Builder<'_>>();
        // value.capnp_serialize(&mut root);
        // let mut buffer = Vec::new();
        // capnp::serialize::write_message(&mut buffer, &message)
        //     .map_err(|e| SerializationError::SerializationFailed(e.to_string()))?;
        // Ok(buffer)
        //
        // Until the trait is refactored and the proc-macro implemented,
        // fall back to the serde_json bridge with a clear diagnostic message.
        serde_json::to_vec(value).map_err(|e| {
            SerializationError::SerializationFailed(format!(
                "Temporary serde_json bridge failed: {}",
                e
            ))
        })
    }

    fn deserialize<T>(data: &[u8]) -> Result<T, SerializationError>
    where
        T: serde::de::DeserializeOwned,
    {
        // FIXME: Temporary serde_json bridge for deserialization. See the
        // comment in `serialize` for the intended proc-macro-based design.
        serde_json::from_slice(data).map_err(|e| {
            SerializationError::DeserializationFailed(format!(
                "Temporary serde_json bridge failed: {}",
                e
            ))
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

// When capnproto feature is enabled, include generated bindings and provide a
// convenience function to serialize the example `PluginExample` struct defined
// in `schemas/example.capnp`.
#[cfg(feature = "capnproto")]
mod generated_adapter {
    use super::*;

    // Only include the generated bindings when the build script was able to
    // run capnpc and set the `capnp_generated` cfg. If codegen was skipped,
    // avoid a hard include! that would cause a compile error.
    #[cfg(capnp_generated)]
    mod gen {
        include!(concat!(env!("OUT_DIR"), "/example_capnp.rs"));
    }

    /// Build and serialize a `PluginExample` message using the generated API.
    /// If codegen didn't run, this function returns an explicit error.
    #[allow(dead_code)]
    pub fn serialize_plugin_example(
        id: &str,
        value: i64,
        payload: &[u8],
    ) -> Result<Vec<u8>, SerializationError> {
        // Two mutually-exclusive cfg blocks so the compiler doesn't see
        // an unconditional `return` followed by more code (which triggers
        // an "unreachable expression" warning when `capnp_generated` is set).
        #[cfg(capnp_generated)]
        {
            let mut message = capnp::message::Builder::new_default();

            // Initialize root as the generated PluginExample struct
            let mut root = message.init_root::<gen::plugin_example::Builder>();
            root.set_id(id);
            root.set_value(value);
            root.set_payload(payload);

            // Serialize into canonical Cap'n Proto words
            return Ok(capnp::serialize::write_message_to_words(&message));
        }

        #[cfg(not(capnp_generated))]
        {
            // If codegen did not run and bindings are unavailable, return a clear error
            // with actionable next steps. Include a pointer to OUT_DIR so CI logs
            // and developers can quickly inspect the generated artifacts directory.
            let out = env!("OUT_DIR");
            return Err(SerializationError::SerializationFailed(format!(
                "capnp codegen bindings not available; expected generated bindings in OUT_DIR='{}'. Install the `capnp` compiler (https://capnproto.org/install.html), enable the `capnproto` feature, and re-run the build. If issues persist, try `cargo clean` before rebuilding to clear stale artifacts.",
                out
            )));
        }
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

    // Smoke test: when both the `capnproto` feature and the `capnp_generated`
    // cfg are present, verify the build produced the generated Rust binding
    // for `schemas/example.capnp` and placed it in OUT_DIR. This fails fast in
    // CI when codegen didn't run or normalization didn't place the file where
    // the proc-macro/include! expects it.
    #[test]
    #[cfg(all(feature = "capnproto", capnp_generated))]
    fn test_generated_binding_present() {
        use std::path::Path;

        let p = Path::new(env!("OUT_DIR")).join("example_capnp.rs");
        assert!(p.exists(), "Expected generated binding at {:?}", p);
    }
}
