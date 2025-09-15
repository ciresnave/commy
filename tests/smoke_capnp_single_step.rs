use commy_common::WithUniqueId;
use commy_macro::create_writer;

// Minimal struct to exercise the proc-macro single-step codegen path.
// Provide a filename so the attribute macro's validation passes and we can exercise
// the codegen paths. The proc-macro requires a filename parameter for writer output.
#[create_writer(filename = "smoke_capnp_example.mmap")]
pub struct SmokeCapnpExample {
    pub id: u64,
    pub payload: Vec<u8>,
}

#[test]
fn smoke_single_step_compile() {
    // The proc-macro should emit schema constants and/or generate the capnp bindings.
    // We don't require capnp locally here; the goal is to ensure macro expansion succeeds
    // far enough to produce the public API. Calling next_id ensures the trait is usable.
    let _ = SmokeCapnpExample::next_id();
    // If capnp is installed and codegen succeeds we should also be able to reference
    // generated types; but the above is enough for compile-time smoke verify.
}
