// This module is a compatibility shim. The canonical, comprehensive
// error types live in `src/error.rs` (module `crate::error`). Many
// internal modules historically referenced `crate::errors::CommyError`.
// To avoid duplicate definitions we re-export the canonical types
// here so both paths remain valid during a conservative migration.

pub use crate::error::{CommyError, CommyResult, ErrorContext};
