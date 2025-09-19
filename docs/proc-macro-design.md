# Proc-macro Design: Generated Zero-Copy Serializers

This document sketches the design for a proc-macro that generates zero-copy
serializers/deserializers for Commy's core types (Cap'n Proto and rkyv).

Goals

- Replace the current serde-bound `SerializationBackend` usage for
  high-performance paths.
- Emit per-type Cap'n Proto schema and a Rust builder/reader pair.
- Keep generated code small and idiomatic; avoid runtime reflection.

High-level approach

- Provide a proc-macro attribute (e.g. `#[derive(CommySchema)]`) that:
  - Emits a `CapnpSchema` trait impl with associated `Builder<'a>` and
    `Reader<'a>` types (or type aliases into generated capnp types).
  - Emits `capnp` schema text (string constant) and optionally writes a
    `.capnp` to `OUT_DIR` so `capnpc` can generate language bindings.
  - Emits `rkyv` serializer helpers where requested.

Minimal generated trait contract (draft)

trait CapnpSchema {
    /// Builder type for writing into a Cap'n Proto message root.
    type Builder<'a>;
    /// Reader type for reading from a Cap'n Proto message root.
    type Reader<'a>;

    /// Serialize `self` into the provided builder.
    fn capnp_serialize<'a>(&self, builder: &mut Self::Builder<'a>);

    /// Deserialize from a reader into `Self`.
    fn capnp_deserialize<'a>(reader: Self::Reader<'a>) -> Result<Self, String>
    where
        Self: Sized;
}

Example of generated code for a simple struct `Foo { id: u64, name: String }`

// in generated module
pub const FOO_CAPNP_SCHEMA: &str = r#"
@0x...;
struct Foo {
  id @0: UInt64;
  name @1: Text;
}
"#;

impl CapnpSchema for Foo {
    type Builder<'a> = gen::foo::Builder<'a>;
    type Reader<'a> = gen::foo::Reader<'a>;

    fn capnp_serialize<'a>(&self, b: &mut Self::Builder<'a>) {
        b.set_id(self.id);
        b.set_name(&self.name);
    }

    fn capnp_deserialize<'a>(r: Self::Reader<'a>) -> Result<Self, String> {
        Ok(Foo { id: r.get_id(), name: r.get_name().map(|s| s.to_string()).unwrap_or_default() })
    }
}

Error model and testing

- Generated deserialization should return Result<T, String> or a small
  error enum that maps to `SerializationError` at call sites.
- Tests:
  - Unit tests for generated code correctness (round-trip simple types).
  - Integration tests: ensure generated `.capnp` files compile with `capnpc`.
  - FFI/unsafe tests: feed malformed data into readers to validate safe
    failure modes.

Migration path

- Initially generate shim impls that delegate to serde for types that
  derive both `Serialize` and `CommySchema`. This speeds migration.
- Gradually convert core types to the generated trait and deprecate the
  serde-backed path.

Open questions

- How to represent optional fields and unions idiomatically in the
  generated API (use Option<T> mapping).
- Interop with rkyv: consider a parallel `RkyvSchema` helper or have the
  proc-macro emit both Cap'n Proto and rkyv code paths.

Next steps

1. Author a small prototype macro that emits schema text and trivial
   builder/reader wrappers for one example type.
2. Wire build.rs to run `capnpc` and generate language bindings in CI.
3. Replace a single internal type's backend to exercise codegen and
   measure performance.
