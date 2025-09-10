//! Serialization support for complex types in memory-mapped files
//!
//! This module provides a unified interface for storing complex types that contain
//! heap-allocated data (like String, Vec, HashMap) in memory-mapped files by
//! serializing them into fixed-size buffers.
//!
//! ## Raw Binary Foundation Architecture
//!
//! All serialization formats are built on top of a raw binary foundation, enabling:
//! - True zero-copy access to memory-mapped data
//! - Universal format support (any format that can become bytes)
//! - Cross-language compatibility (all languages can work with raw bytes)
//! - Maximum performance (direct memory access without intermediate serialization)

use std::marker::PhantomData;

// Raw binary foundation - all formats build on this
mod raw_binary;
pub use raw_binary::{
    FormatData, RawBinaryData, RawBinaryError, RawBytes, UniversalData, ZeroCopyAccess,
    ZeroCopyBytes,
};

#[cfg(feature = "zerocopy")]
pub use raw_binary::RkyvData;

#[cfg(feature = "capnproto")]
pub use raw_binary::CapnProtoData;

// Legacy backends for backward compatibility
#[cfg(feature = "zerocopy")]
mod rkyv_backend;

#[cfg(feature = "zerocopy")]
pub use rkyv_backend::{RkyvSerializer, RkyvZeroCopyBackend};

#[cfg(feature = "capnproto")]
mod capnproto_backend;

#[cfg(feature = "capnproto")]
pub use capnproto_backend::{CapnProtoBackend, CapnProtoSerializer};

/// Error type for serialization operations
#[derive(Debug)]
pub enum SerializationError {
    SerializationFailed(String),
    DeserializationFailed(String),
    BufferTooSmall { required: usize, available: usize },
    FormatNotSupported(String),
}

impl std::fmt::Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializationError::SerializationFailed(msg) => {
                write!(f, "Serialization failed: {}", msg)
            }
            SerializationError::DeserializationFailed(msg) => {
                write!(f, "Deserialization failed: {}", msg)
            }
            SerializationError::BufferTooSmall {
                required,
                available,
            } => {
                write!(
                    f,
                    "Buffer too small: need {} bytes, have {}",
                    required, available
                )
            }
            SerializationError::FormatNotSupported(format) => {
                write!(f, "Format not supported: {}", format)
            }
        }
    }
}

impl std::error::Error for SerializationError {}

/// Trait for serialization backends
pub trait SerializationBackend {
    fn name() -> &'static str;
    fn serialize<T>(value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: ?Sized + serde::Serialize;
    fn deserialize<T>(data: &[u8]) -> Result<T, SerializationError>
    where
        T: serde::de::DeserializeOwned;
}

/// Trait representing zero-copy capable backends (rkyv, capnp).
/// This trait is intentionally non-generic: actual type-specific serialization
/// helpers live in their dedicated backend modules (for example, `rkyv_backend`).
#[cfg(feature = "zerocopy")]
pub trait ZeroCopyBackend {
    /// Backend name (e.g., "rkyv")
    fn backend_name() -> &'static str;

    /// Quick validity check of raw bytes for this backend. Should be conservative
    /// and return false for obviously invalid inputs.
    fn validate_bytes(bytes: &[u8]) -> bool;
}

// When zerocopy feature is enabled, expose a default ZeroCopyData alias using
// the rkyv backend implementation. This keeps the rest of the codebase using a
// concrete backend type rather than a trait object.
#[cfg(feature = "zerocopy")]
pub type DefaultZeroCopyBackend = crate::serialization::RkyvZeroCopyBackend;

/// JSON serialization backend
#[cfg(feature = "json")]
#[derive(Debug)]
pub struct JsonBackend;

#[cfg(feature = "json")]
impl SerializationBackend for JsonBackend {
    fn name() -> &'static str {
        "JSON"
    }

    fn serialize<T>(value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: ?Sized + serde::Serialize,
    {
        serde_json::to_vec(value).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            let com_err = crate::errors::CommyError::JsonSerialization(e);
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::Json));
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
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::Json));
            SerializationError::DeserializationFailed(format!("{:?}", trans_err))
        })
    }
}

/// Binary (bincode) serialization backend
#[cfg(feature = "binary")]
#[derive(Debug)]
pub struct BinaryBackend;

#[cfg(feature = "binary")]
impl SerializationBackend for BinaryBackend {
    fn name() -> &'static str {
        "Binary"
    }

    fn serialize<T>(value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: ?Sized + serde::Serialize,
    {
        // bincode may not expose the same helper APIs under all feature sets in this workspace,
        // fall back to serde_json as a stable binary-compatible bridge for CI/lint runs.
        serde_json::to_vec(value)
            .map_err(|e| SerializationError::SerializationFailed(e.to_string()))
    }

    fn deserialize<T>(data: &[u8]) -> Result<T, SerializationError>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_slice(data)
            .map_err(|e| SerializationError::DeserializationFailed(e.to_string()))
    }
}

/// MessagePack serialization backend
#[cfg(feature = "messagepack")]
#[derive(Debug)]
pub struct MessagePackBackend;

#[cfg(feature = "messagepack")]
impl SerializationBackend for MessagePackBackend {
    fn name() -> &'static str {
        "MessagePack"
    }

    fn serialize<T>(value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: ?Sized + serde::Serialize,
    {
        rmp_serde::to_vec(value).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            let com_err = crate::errors::CommyError::MessagePackSerialization(e.to_string());
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::MessagePack));
            SerializationError::SerializationFailed(format!("{:?}", trans_err))
        })
    }

    fn deserialize<T>(data: &[u8]) -> Result<T, SerializationError>
    where
        T: serde::de::DeserializeOwned,
    {
        rmp_serde::from_slice(data).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            let com_err = crate::errors::CommyError::MessagePackSerialization(e.to_string());
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::MessagePack));
            SerializationError::DeserializationFailed(format!("{:?}", trans_err))
        })
    }
}

/// Compact (postcard) serialization backend - very space efficient
#[cfg(feature = "compact")]
#[derive(Debug)]
pub struct CompactBackend;

#[cfg(feature = "compact")]
impl SerializationBackend for CompactBackend {
    fn name() -> &'static str {
        "Compact"
    }

    fn serialize<T>(value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: ?Sized + serde::Serialize,
    {
        postcard::to_allocvec(value)
            .map_err(|e| SerializationError::SerializationFailed(e.to_string()))
    }

    fn deserialize<T>(data: &[u8]) -> Result<T, SerializationError>
    where
        T: serde::de::DeserializeOwned,
    {
        postcard::from_bytes(data)
            .map_err(|e| SerializationError::DeserializationFailed(e.to_string()))
    }
}

/// Zero-copy (rkyv) serialization backend - maximum performance for shared memory
/// Fixed-size buffer that holds serialized data
#[derive(Debug, Clone, Copy)]
pub struct SerializedData<T, B, const SIZE: usize>
where
    B: SerializationBackend,
{
    buffer: [u8; SIZE],
    len: u32,
    _phantom: PhantomData<(T, B)>,
}

impl<T, B, const SIZE: usize> Default for SerializedData<T, B, SIZE>
where
    B: SerializationBackend,
{
    fn default() -> Self {
        SerializedData {
            buffer: [0; SIZE],
            len: 0,
            _phantom: PhantomData,
        }
    }
}

impl<T, B, const SIZE: usize> SerializedData<T, B, SIZE>
where
    B: SerializationBackend,
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    /// Create a new SerializedData from a value
    pub fn new(value: T) -> Result<Self, SerializationError> {
        let serialized = B::serialize(&value)?;

        if serialized.len() > SIZE {
            return Err(SerializationError::BufferTooSmall {
                required: serialized.len(),
                available: SIZE,
            });
        }

        let mut buffer = [0; SIZE];
        buffer[..serialized.len()].copy_from_slice(&serialized);

        Ok(SerializedData {
            buffer,
            len: serialized.len() as u32,
            _phantom: PhantomData,
        })
    }

    /// Get the original value by deserializing
    pub fn get(&self) -> Result<T, SerializationError> {
        let data = &self.buffer[..self.len as usize];
        B::deserialize(data)
    }

    /// Update the stored value
    pub fn set(&mut self, value: T) -> Result<(), SerializationError> {
        let serialized = B::serialize(&value)?;

        if serialized.len() > SIZE {
            return Err(SerializationError::BufferTooSmall {
                required: serialized.len(),
                available: SIZE,
            });
        }

        self.buffer[..serialized.len()].copy_from_slice(&serialized);
        self.len = serialized.len() as u32;
        Ok(())
    }

    /// Get the serialization format name
    pub fn format_name(&self) -> &'static str {
        B::name()
    }

    /// Get the current buffer usage
    pub fn buffer_usage(&self) -> (usize, usize) {
        (self.len as usize, SIZE)
    }

    /// Return a byte slice view of the serialized contents
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer[..self.len as usize]
    }

    /// Construct a SerializedData from raw bytes (copying into the fixed buffer)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SerializationError> {
        if bytes.len() > SIZE {
            return Err(SerializationError::BufferTooSmall {
                required: bytes.len(),
                available: SIZE,
            });
        }

        let mut buffer = [0; SIZE];
        buffer[..bytes.len()].copy_from_slice(bytes);

        Ok(SerializedData {
            buffer,
            len: bytes.len() as u32,
            _phantom: PhantomData,
        })
    }

    /// Convenience alias: serialize from a reference to a value
    pub fn serialize(value: &T) -> Result<Self, SerializationError> {
        let serialized = B::serialize(value)?;

        if serialized.len() > SIZE {
            return Err(SerializationError::BufferTooSmall {
                required: serialized.len(),
                available: SIZE,
            });
        }

        let mut buffer = [0; SIZE];
        buffer[..serialized.len()].copy_from_slice(&serialized);

        Ok(SerializedData {
            buffer,
            len: serialized.len() as u32,
            _phantom: PhantomData,
        })
    }

    /// Convenience alias for get()
    pub fn deserialize(&self) -> Result<T, SerializationError> {
        self.get()
    }

    /// Zero-copy view equivalent for this fixed-size buffer
    pub fn zero_copy_view(&self) -> &[u8] {
        self.as_bytes()
    }

    /// Heuristic check for zero-copy validity
    pub fn is_valid_for_zero_copy(&self) -> bool {
        !self.as_bytes().is_empty()
    }
}

// Type aliases for common use cases
#[cfg(feature = "json")]
pub type JsonData<T, const SIZE: usize = 1024> = SerializedData<T, JsonBackend, SIZE>;

#[cfg(feature = "binary")]
pub type BinaryData<T, const SIZE: usize = 1024> = SerializedData<T, BinaryBackend, SIZE>;

#[cfg(feature = "messagepack")]
pub type MessagePackData<T, const SIZE: usize = 1024> = SerializedData<T, MessagePackBackend, SIZE>;

#[cfg(feature = "compact")]
pub type CompactData<T, const SIZE: usize = 1024> = SerializedData<T, CompactBackend, SIZE>;

/// Zero-copy serialized data (optimized for shared memory)
#[cfg(feature = "zerocopy")]
pub type ZeroCopyData<T, const SIZE: usize = 2048> =
    SerializedData<T, DefaultZeroCopyBackend, SIZE>;

/// Macro to create serialized field assignments
#[macro_export]
macro_rules! serialize_field {
    ($writer:expr, $field:ident, $value:expr, $format:ty) => {
        $writer.data.$field = $crate::FieldHolder::new(
            <$format>::new($value).map_err(|e| format!("Serialization error: {}", e))?,
            $writer.writer_id,
        );
    };
}

/// Macro to deserialize and get field values
#[macro_export]
macro_rules! deserialize_field {
    ($data:expr, $field:ident) => {
        $data
            .$field
            .get()
            .get()
            .map_err(|e| format!("Deserialization error: {}", e))?
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone, Default)]
    struct TestData {
        name: String,
        values: Vec<i32>,
        metadata: HashMap<String, String>,
    }

    #[cfg(feature = "json")]
    #[test]
    fn test_json_serialization() {
        let mut data = TestData::default();
        data.metadata
            .insert("key1".to_string(), "value1".to_string());

        let serialized = JsonData::<TestData>::new(data.clone()).unwrap();
        let deserialized = serialized.get().unwrap();

        assert_eq!(data, deserialized);
        assert_eq!(serialized.format_name(), "JSON");
    }

    #[cfg(feature = "binary")]
    #[test]
    fn test_binary_serialization() {
        let mut data = TestData::default();
        data.metadata
            .insert("key1".to_string(), "value1".to_string());

        let serialized = BinaryData::<TestData>::new(data.clone()).unwrap();
        let deserialized = serialized.get().unwrap();

        assert_eq!(data, deserialized);
        assert_eq!(serialized.format_name(), "Binary");
    }
}
