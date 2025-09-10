//! FFI (Foreign Function Interface) module for multi-language SDK support
//!
//! This module provides C-compatible interfaces that enable binding from
//! various programming languages including Python, JavaScript/Node.js, Go,
//! Java, and .NET.
//!
//! The FFI layer is designed with these principles:
//! - **Stable ABI**: C-compatible function signatures that won't break between versions
//! - **Memory Safety**: Proper ownership transfer and lifetime management
//! - **Error Handling**: Consistent error codes and optional error message retrieval
//! - **Performance**: Zero-copy operations where possible, minimal overhead
//! - **Thread Safety**: All functions are thread-safe and can be called from any thread

// The minimal FFI implementation is useful for older demos/tests but defines
// many of the same extern "C" symbols as the canonical implementations.
// Gate it behind an optional feature so it won't be compiled by default and
// cause duplicate symbol/linker errors (for example: commy_configure_mesh).
pub mod memory;
#[cfg(feature = "ffi_minimal")]
pub mod minimal;
pub mod types;
pub mod working_sync;
// Note: legacy `src/ffi/working.rs` remains on-disk and is feature-gated inside
// the file itself with `#![cfg(feature = "ffi_legacy")]`. We intentionally do
// not re-export it here so legacy FFI isn't compiled by default.

// Use the working synchronous implementation and expose types for consumers
// pub use minimal::*;
pub use memory::*;
pub use types::*;
pub use working_sync::*;

// TODO: Fix FFI tests to match current interface
// #[cfg(test)]
// mod tests;
