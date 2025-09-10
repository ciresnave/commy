//! Raw Binary Foundation - The Universal Serialization Interface
//!
//! This module provides the foundational raw binary interface that all serialization
//! formats build upon. This enables true zero-copy access, universal format support,
//! and maximum performance.

// std::borrow::Cow was previously included but is unused; removed to silence warnings

/// Error type for raw binary operations
#[derive(Debug)]
pub enum RawBinaryError {
    InvalidData(String),
    InsufficientData { expected: usize, available: usize },
    ConversionFailed(String),
}

impl std::fmt::Display for RawBinaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RawBinaryError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            RawBinaryError::InsufficientData {
                expected,
                available,
            } => {
                write!(
                    f,
                    "Insufficient data: expected {} bytes, got {}",
                    expected, available
                )
            }
            RawBinaryError::ConversionFailed(msg) => write!(f, "Conversion failed: {}", msg),
        }
    }
}

impl std::error::Error for RawBinaryError {}

/// The foundation trait: Everything ultimately becomes raw bytes
/// This enables true zero-copy access and universal serialization support
pub trait RawBinaryData {
    /// Get the raw bytes representation
    /// For zero-copy types, this should return a direct reference to the underlying data
    fn as_bytes(&self) -> &[u8];

    /// Create from raw bytes
    /// For zero-copy types, this should interpret the bytes directly without copying
    fn from_bytes(bytes: &[u8]) -> Result<Self, RawBinaryError>
    where
        Self: Sized;

    /// Get the length in bytes
    fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Raw bytes wrapper - the simplest implementation
#[derive(Debug, Clone, PartialEq)]
pub struct RawBytes(Vec<u8>);

impl RawBytes {
    pub fn new(data: Vec<u8>) -> Self {
        RawBytes(data)
    }

    pub fn from_slice(data: &[u8]) -> Self {
        RawBytes(data.to_vec())
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl RawBinaryData for RawBytes {
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, RawBinaryError> {
        Ok(RawBytes(bytes.to_vec()))
    }
}

/// Zero-copy bytes wrapper - for memory-mapped data
#[derive(Debug)]
pub struct ZeroCopyBytes<'a>(&'a [u8]);

impl<'a> ZeroCopyBytes<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        ZeroCopyBytes(data)
    }
}

impl<'a> RawBinaryData for ZeroCopyBytes<'a> {
    fn as_bytes(&self) -> &[u8] {
        self.0
    }

    fn from_bytes(_bytes: &[u8]) -> Result<Self, RawBinaryError>
    where
        Self: Sized,
    {
        Err(RawBinaryError::ConversionFailed(
            "ZeroCopyBytes::from_bytes is unsupported; construct ZeroCopyBytes directly".into(),
        ))
    }
}

/// Format-specific data that builds on raw binary foundation
pub trait FormatData: RawBinaryData {
    /// The format identifier
    fn format_name() -> &'static str;

    /// Serialize typed data to raw bytes
    fn serialize<T>(value: &T) -> Result<Self, RawBinaryError>
    where
        T: ?Sized + serde::Serialize,
        Self: Sized;

    /// Deserialize raw bytes to typed data
    fn deserialize<T>(&self) -> Result<T, RawBinaryError>
    where
        T: serde::de::DeserializeOwned;
}

/// JSON format built on raw binary foundation
#[derive(Debug, Clone)]
pub struct JsonData(RawBytes);

impl RawBinaryData for JsonData {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, RawBinaryError> {
        Ok(JsonData(RawBytes::from_bytes(bytes)?))
    }
}

impl FormatData for JsonData {
    fn format_name() -> &'static str {
        "JSON"
    }

    fn serialize<T>(value: &T) -> Result<Self, RawBinaryError>
    where
        T: serde::Serialize + ?Sized,
    {
        let bytes = serde_json::to_vec(value).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            let com_err = crate::errors::CommyError::JsonSerialization(e);
            // This is JSON data — map with Json format so context is preserved
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::Json));
            RawBinaryError::ConversionFailed(format!("{:?}", trans_err))
        })?;
        Ok(JsonData(RawBytes::new(bytes)))
    }

    fn deserialize<T>(&self) -> Result<T, RawBinaryError>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_slice(self.as_bytes()).map_err(|e| {
            use crate::manager::transport_impl::map_commy_error_to_transport_error;
            use crate::manager::SerializationFormat;
            let com_err = crate::errors::CommyError::JsonSerialization(e);
            // JSON deserialization error — preserve JSON format context
            let trans_err =
                map_commy_error_to_transport_error(com_err, Some(SerializationFormat::Json));
            RawBinaryError::ConversionFailed(format!("{:?}", trans_err))
        })
    }
}

/// rkyv format built on raw binary foundation - TRUE ZERO COPY
#[cfg(feature = "zerocopy")]
#[derive(Debug)]
pub struct RkyvData(RawBytes);

#[cfg(feature = "zerocopy")]
impl RawBinaryData for RkyvData {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, RawBinaryError> {
        Ok(RkyvData(RawBytes::from_bytes(bytes)?))
    }
}

/// Cap'n Proto format built on raw binary foundation
#[cfg(feature = "capnproto")]
#[derive(Debug)]
pub struct CapnProtoData(RawBytes);

#[cfg(feature = "capnproto")]
impl RawBinaryData for CapnProtoData {
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, RawBinaryError> {
        Ok(CapnProtoData(RawBytes::from_bytes(bytes)?))
    }
}

/// Universal data wrapper that can hold any format
#[derive(Debug)]
pub enum UniversalData {
    Raw(RawBytes),
    Json(JsonData),
    #[cfg(feature = "zerocopy")]
    Rkyv(RkyvData),
    #[cfg(feature = "capnproto")]
    CapnProto(CapnProtoData),
}

impl RawBinaryData for UniversalData {
    fn as_bytes(&self) -> &[u8] {
        match self {
            UniversalData::Raw(data) => data.as_bytes(),
            UniversalData::Json(data) => data.as_bytes(),
            #[cfg(feature = "zerocopy")]
            UniversalData::Rkyv(data) => data.as_bytes(),
            #[cfg(feature = "capnproto")]
            UniversalData::CapnProto(data) => data.as_bytes(),
        }
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, RawBinaryError> {
        Ok(UniversalData::Raw(RawBytes::from_bytes(bytes)?))
    }
}

impl UniversalData {
    /// Create from any format-specific data
    pub fn from_format<F: FormatData>(data: F) -> Self {
        match F::format_name() {
            "JSON" => UniversalData::Json(JsonData::from_bytes(data.as_bytes()).unwrap()),
            #[cfg(feature = "zerocopy")]
            "rkyv" => UniversalData::Rkyv(RkyvData::from_bytes(data.as_bytes()).unwrap()),
            #[cfg(feature = "capnproto")]
            "Cap'n Proto" => {
                UniversalData::CapnProto(CapnProtoData::from_bytes(data.as_bytes()).unwrap())
            }
            _ => UniversalData::Raw(RawBytes::from_bytes(data.as_bytes()).unwrap()),
        }
    }

    /// Get the format name
    pub fn format_name(&self) -> &'static str {
        match self {
            UniversalData::Raw(_) => "Raw",
            UniversalData::Json(_) => "JSON",
            #[cfg(feature = "zerocopy")]
            UniversalData::Rkyv(_) => "rkyv",
            #[cfg(feature = "capnproto")]
            UniversalData::CapnProto(_) => "Cap'n Proto",
        }
    }
}

/// Zero-copy access interface for memory-mapped data
pub trait ZeroCopyAccess: RawBinaryData {
    /// Get a zero-copy view of the data without deserialization
    /// This enables direct access to archived/structured data in memory
    fn zero_copy_view(&self) -> &[u8] {
        self.as_bytes()
    }

    /// Check if data is valid for zero-copy access
    fn is_valid_for_zero_copy(&self) -> bool {
        // Default: non-empty
        !self.is_empty()
    }
}

#[cfg(feature = "zerocopy")]
impl ZeroCopyAccess for RkyvData {
    fn is_valid_for_zero_copy(&self) -> bool {
        // Conservative validity check for rkyv-backed data
        !self.as_bytes().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_bytes() {
        let data = vec![1, 2, 3, 4, 5];
        let raw = RawBytes::new(data.clone());

        assert_eq!(raw.as_bytes(), &data);
        assert_eq!(raw.len(), 5);

        let reconstructed = RawBytes::from_bytes(raw.as_bytes()).unwrap();
        assert_eq!(reconstructed, raw);
    }

    #[test]
    fn test_zero_copy_bytes() {
        let data = [1, 2, 3, 4, 5];
        let zero_copy = ZeroCopyBytes::new(&data);

        assert_eq!(zero_copy.as_bytes(), &data);
        assert_eq!(zero_copy.len(), 5);
    }

    #[test]
    fn test_json_format() {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestData {
            name: String,
            value: i32,
        }

        let test_data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        let json_data = JsonData::serialize(&test_data).unwrap();
        assert_eq!(JsonData::format_name(), "JSON");

        let reconstructed: TestData = json_data.deserialize().unwrap();
        assert_eq!(reconstructed, test_data);

        // Test raw binary foundation
        let raw_bytes = json_data.as_bytes();
        let from_raw = JsonData::from_bytes(raw_bytes).unwrap();
        let final_data: TestData = from_raw.deserialize().unwrap();
        assert_eq!(final_data, test_data);
    }

    #[test]
    fn test_universal_data() {
        let raw_data = vec![1, 2, 3, 4, 5];
        let universal = UniversalData::Raw(RawBytes::new(raw_data.clone()));

        assert_eq!(universal.as_bytes(), &raw_data);
        assert_eq!(universal.format_name(), "Raw");

        let from_bytes = UniversalData::from_bytes(&raw_data).unwrap();
        assert_eq!(from_bytes.as_bytes(), &raw_data);
    }
}
