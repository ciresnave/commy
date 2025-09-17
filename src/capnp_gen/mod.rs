// This module is intentionally minimal. The `build.rs` script will run
// capnpc on .capnp files in `schemas/` and place generated Rust modules into
// the crate's OUT_DIR. Consumers can `include!(concat!(env!("OUT_DIR"), "/<schema>.rs"))`
// or this module can be expanded to re-export generated symbols as needed.

#[cfg(feature = "capnproto")]
pub mod generated {
    // Example: the build script will place generated files like `example_capnp.rs`
    // into OUT_DIR; users can `include!` those here when needed.
}

// Small helper to include a generated file by basename. This keeps the
// reference in one place and is noop when the feature is not enabled.
#[macro_export]
macro_rules! include_generated_capnp {
    ($basename:expr) => {
        #[cfg(feature = "capnproto")]
        {
            include!(concat!(env!("OUT_DIR"), "/", $basename, "_capnp.rs"));
        }
        #[cfg(not(feature = "capnproto"))]
        {
            // No-op when capnproto feature not enabled.
        }
    };
}
