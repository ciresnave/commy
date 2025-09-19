// This module is intentionally minimal. The `build.rs` script will run
// capnpc on .capnp files in `schemas/` and place generated Rust modules into
// the crate's OUT_DIR. Consumers can `include!(concat!(env!("OUT_DIR"), "/<schema>.rs"))`
// or this module can be expanded to re-export generated symbols as needed.

#[cfg(feature = "capnproto")]
pub mod generated {
    // Example: the build script will place generated files like `example_capnp.rs`
    // into OUT_DIR; users can `include!` those here when needed.
}

// Small helper to include a generated file by basename. Emit an item-level
// module so the macro expansion is syntactically valid at module scope.
#[macro_export]
macro_rules! include_generated_capnp {
    ($basename:expr) => {
        #[allow(non_camel_case_types, unused_imports, dead_code)]
        #[cfg(feature = "capnproto")]
        mod __commy_include_generated_bindings_for_ {
            // The module name above is intentionally anonymous in the token
            // stream; we immediately re-export its contents by including the
            // generated file inside. This keeps the invocation safe at
            // module scope and avoids emitting bare blocks guarded by cfg.
            include!(concat!(env!("OUT_DIR"), "/", $basename, "_capnp.rs"));
        }
        #[cfg(not(feature = "capnproto"))]
        #[allow(dead_code)]
        const _: () = { () };
    };
}
