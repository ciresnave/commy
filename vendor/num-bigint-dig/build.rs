fn main() {
    // Inform rustc's check-cfg that `has_i128` is an expected cfg used in the
    // vendored crate. This prevents the "unexpected `cfg` condition name: `has_i128`"
    // warnings emitted by recent rustc versions when checking cfg attributes.
    println!("cargo:rustc-check-cfg=cfg(has_i128)");
}
