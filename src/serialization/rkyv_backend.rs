use crate::serialization::ZeroCopyBackend;
use crate::serialization::{SerializationBackend, SerializationError};

/// Deprecated generic serialization adapter (serde bridge)
#[derive(Debug)]
pub struct ZeroCopyAdapter;

impl SerializationBackend for ZeroCopyAdapter {
    fn name() -> &'static str {
        "ZeroCopyAdapter"
    }

    fn serialize<T>(value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: ?Sized + serde::Serialize,
    {
        // Bridge via serde until we implement strong rkyv integration
        serde_json::to_vec(value).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            // Map serde bridge errors into explicit JsonSerialization so the
            // centralized mapper can preserve the original error details.
            let com_err = crate::errors::CommyError::JsonSerialization(e);
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::ZeroCopy));
            SerializationError::SerializationFailed(format!("{:?}", trans_err))
        })
    }

    fn deserialize<T>(data: &[u8]) -> Result<T, SerializationError>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_slice(data).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            let com_err = crate::errors::CommyError::JsonSerialization(e);
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::ZeroCopy));
            SerializationError::DeserializationFailed(format!("{:?}", trans_err))
        })
    }
}

/// True Zero-copy backend marker for rkyv. Implement backend-specific helpers
/// here. This trait intentionally does not try to force T: rkyv::Archive at the
/// generic SerializationBackend level — that would be overly restrictive for the
/// rest of the codebase. Instead, lower-level code (proc-macro and generated
/// writers) will call into rkyv-specific APIs directly.
pub struct RkyvZeroCopyBackend;

impl ZeroCopyBackend for RkyvZeroCopyBackend {
    fn backend_name() -> &'static str {
        "rkyv"
    }

    fn validate_bytes(_bytes: &[u8]) -> bool {
        // Conservative: ensure non-empty; detailed validation will be done by
        // concrete rkyv-aware code that knows the archived type.
        !_bytes.is_empty()
    }
}

// Backwards-compatible blanket impl so examples using RkyvZeroCopyBackend as a
// SerializationBackend compile. This delegates to the ZeroCopyAdapter serde bridge
// for now.
impl crate::SerializationBackend for RkyvZeroCopyBackend {
    fn name() -> &'static str {
        ZeroCopyAdapter::name()
    }

    fn serialize<T>(value: &T) -> Result<Vec<u8>, crate::SerializationError>
    where
        T: ?Sized + serde::Serialize,
    {
        ZeroCopyAdapter::serialize(value)
    }

    fn deserialize<T>(data: &[u8]) -> Result<T, crate::SerializationError>
    where
        T: serde::de::DeserializeOwned,
    {
        ZeroCopyAdapter::deserialize(data)
    }
}

/// Backwards-compatible serializer wrapper exported by the module
pub struct RkyvSerializer;

impl RkyvSerializer {
    pub fn serialize<T>(value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: ?Sized + serde::Serialize,
    {
        ZeroCopyAdapter::serialize(value)
    }

    pub fn deserialize<T>(data: &[u8]) -> Result<T, SerializationError>
    where
        T: serde::de::DeserializeOwned,
    {
        ZeroCopyAdapter::deserialize(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u64,
        name: String,
        values: Vec<i32>,
    }

    #[test]
    fn test_zero_copy_backend() {
        let data = TestData {
            id: 42,
            name: "test".to_string(),
            values: vec![1, 2, 3, 4, 5],
        };

        // Use the serde-bridge adapter to verify round-trip through the generic adapter
        let serialized = ZeroCopyAdapter::serialize(&data).expect("Serialization failed");
        let deserialized =
            ZeroCopyAdapter::deserialize::<TestData>(&serialized).expect("Deserialization failed");

        assert_eq!(data, deserialized);
    }
}
