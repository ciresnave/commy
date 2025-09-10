# Integration Plan: Hybrid Compile-time + Runtime Type Introspection and Plugin System

This document describes a step-by-step plan to integrate a hybrid type-introspection and plugin system into Commy. The goal is to allow:

- Automatic schema generation for user types
- Zero-copy serialization (rkyv) and direct writes into existing buffers (e.g., mmapped files)
- Polyglot schemas (Cap'n Proto) generated from the same type definitions when required
- Runtime addition of types through dynamically loaded plugins (Windows .dll / Unix .so)

The approach balances compile-time code generation (proc-macro) for maximal performance with a small, stable runtime plugin ABI to allow dynamic types.

## Contract (Inputs / Outputs / Error Modes)

- Inputs:
  - A user Rust type (struct/enum) annotated with the proc-macro to opt-in to Commy's codegen.
  - Optionally, a compiled plugin (shared library) exposing one or more type descriptors for runtime-loading.

- Outputs:
  - For compile-time types: generated rkyv `Archived` layout plus a zero-copy writer function that writes into an arbitrary buffer and returns size written.
  - For Cap'n Proto: generated schema text associated with the type (if capnp feature enabled) and an optional serializer that can write capnp bytes directly.
  - At runtime: a registry keyed by type name and schema-hash that maps to serializer/deserializer entrypoints.

- Error modes and decisions:
  - If a plugin's schema/hash conflicts with a compiled-in type, Commy will prefer exact-match by schema-hash and expose a clear error on mismatch.
  - When a plugin cannot provide a zero-copy writer for a requested format, Commy will fall back to a canonical (serde) path if available and logged as a performance downgrade.

## High-level Architecture

1. Proc-macro (compile-time)
   - A proc-macro in `commy_macro` will generate:
     - rkyv `Archived` type definitions and impls (via rkyv derive or direct codegen) so the archived layout is known and compact.
     - A function with extern "Rust" signature (regular Rust function) that serializes the type into a provided buffer (slice or raw pointer + len) and returns bytes written or error.
     - A function that produces Cap'n Proto schema text (optional, behind a feature flag) for external polyglot consumers.
   - Generated functions are intended for compiled-in types and called directly by Commy; they can use unsafe/zero-copy paths into an MMAP region.

2. Runtime plugin (dynamic)
   - Plugins are compiled as `cdylib` (or `dylib` if appropriate) and expose a small C-compatible ABI to register their types and serializers with Commy.
   - Commy will load plugins using `libloading` and call a well-known symbol, e.g. `com_my_plugin_register`.
   - Each plugin provides one or more `PluginTypeDescriptor` structures (C-compatible), each containing:
     - type name (null-terminated C string)
     - schema hash (u64 or 16-byte hash)
     - supported formats bitflags (rkyv, capnp, json, etc.)
     - pointers to C-callable functions for serialize/deserialize where applicable
     - opaque plugin state pointer (void*) and destructor callback

3. Runtime registry
   - Implement a thread-safe registry (use `dashmap`) keyed by `(&'static str` or `String`) + schema-hash -> TypeEntry.
   - TypeEntry contains available formats and function pointers (Rust closures boxed behind thin wrappers) or FFI pointers for plugin types.
   - Registry used by transports and SerializationBackend to find the appropriate serializer/deserializer.

4. Serialization backend integration
   - Existing backends (rkyv, capnp, serde) will be adapted to attempt direct zero-copy writes using the registry's writer functions when available.
   - If not available, backend falls back to an in-process serializer (serde/bincode) or to a plugin-provided fallback.

## Plugin ABI (C-compatible)

Design goals:

- Minimal surface area.
- No Rust generics or trait objects in the exported ABI.
- Use plain C types (u8, u64, const char*, void*) and function pointers.
- Plugins are responsible for building any bridging code (e.g., converting their internal Rust types to the expected format). Commy will only hold opaque pointers.

Suggested C-like declarations (for documentation):

```c
// bitflags for supported formats
typedef uint32_t commy_formats_t;

typedef struct {
    const char* type_name;           // null-terminated
    uint64_t schema_hash;           // stable hash of schema text
    commy_formats_t formats;        // bitflags

    // Serialize into a caller-owned buffer. Returns bytes_written or 0 on error.
    // signature: size_t (*serialize_into)(void* ctx, const void* untyped_ptr, uint8_t* out_buf, size_t out_len);
    size_t (*serialize_into)(void* ctx, const void* typed_ptr, uint8_t* out_buf, size_t out_len);

    // Optional: get schema text (null-terminated). Caller will not free. Can be NULL.
    const char* (*get_schema_text)(void* ctx);

    // Optional: cleanup for ctx
    void (*ctx_drop)(void* ctx);

    void* ctx; // opaque plugin state
} PluginTypeDescriptor;

// Plugin registration entrypoint
void com_my_plugin_register(const PluginTypeDescriptor** descriptors, size_t n);
```

On the Rust side, plugins should expose the `com_my_plugin_register` symbol via `#[no_mangle] pub extern "C" fn com_my_plugin_register(...) { ... }`.

Windows specifics: when producing `.dll`, use `crate-type = ["cdylib"]` and `#[no_mangle] pub extern "C"` to export. Ensure the plugin build produces stable names.

Notes on schema_hash: choose a stable, fast hash (e.g., XXH64 or SipHash with fixed seed) over the schema text. Document precisely how `schema_text` is produced so different compilers/generators produce identical text (whitespace normalization, stable field ordering, type canonicalization).

## Proc-macro contract (commy_macro)

For a user type `T` (e.g., `#[derive(CommyType)]` or `#[commy::generate]`), the macro should generate:

- A module `commy_generated::T` that contains:
  - `pub fn write_to_buffer(value: &T, out: &mut [u8]) -> Result<usize, SerializeError>` — writes bytes (rkyv archived) directly into `out` (no extra allocations) and returns bytes used.
  - `pub fn schema_text() -> &'static str` — Cap'n Proto schema text (if feature enabled) or other schema representation.
  - `pub const SCHEMA_HASH: u64` — precomputed schema hash.

These functions are internal to Commy and callable directly when the type is compiled into the process.

Developer ergonomics: macro should accept options for feature flags, e.g. `#[commy(generate(capnp))]`.

## Registry API (Rust sketch)

pub struct TypeEntry {
    pub type_name: String,
    pub schema_hash: u64,
    pub formats: FormatsBitflags,
    pub writer: Option<fn(&dyn Any, &mut [u8]) -> Result<usize, SerializeError>>,
    // For plugin FFI entries, a thin wrapper will convert to this signature.
}

pub type Registry = DashMap<(String, u64), Arc<TypeEntry>>;

APIs:

- register_compiled_type(entry: TypeEntry)
- register_plugin_type(plugin_desc: PluginTypeDescriptor)
- lookup(type_name: &str, schema_hash: Option<u64>) -> Option<Arc<TypeEntry>>

Concurrency: registry uses dashmap; registration should be idempotent and return existing entry when conflicts occur.

## Loader (libloading) sketch

- Search configured plugin directories (configurable) and platform-specific extension (".dll", ".so", ".dylib").
- For each plugin file: use libloading::Library::new(path) then get the `com_my_plugin_register` symbol.
- Call `com_my_plugin_register` with an array of pointers the plugin provides. For safety, hold the Library instance in the registry to keep symbols alive.

Careful points:

- The plugin must not depend on types that are not ABI-stable across the host and plugin; keep the ABI isolated and C-compatible.
- Preserve Library handles to avoid unloading while entries are used. Provide a mechanism to unload safely if required.

## Testing and Examples

1. Unit tests for proc-macro-generated writer functions (happy path + buffer-too-small).
2. Integration example: compile a binary with a generated type and write directly to an mmapped file using the generated write_to_buffer(). Verify zero-copy access using rkyv's `archived_root` on the mmapped bytes.
3. Plugin example crate: a minimal `cdylib` which defines a type and registers a PluginTypeDescriptor. Add a small example app which loads the plugin and uses the registered serializer to write into a buffer; verify deserialization in the host.
4. Cap'n Proto roundtrip test: generate schema_text from the proc-macro, compile capnp schema for plugin consumer, and round-trip a value via capnp path (if the feature is enabled).

## Edge Cases and Failure Modes

- ABI mismatch between plugin and host: detect by validating schema_hash and refusing to register mismatches.
- Different compilers or macro versions producing different schema_text: mitigate by canonical schema generation rules and stable hashing.
- Plugins that crash on call: isolate plugin calls and return errors; avoid allowing plugin panics to unwind into the host (wrap FFI in catch_unwind or require plugin code to not panic).
- Unloading plugins: hold Library handle while any type entries are registered; provide explicit unregister semantics.

## Build and CI notes

- Add example plugin building to the repo with platform-specific cargo workflows. For Windows, use `cargo build --release --target x86_64-pc-windows-msvc` and produce `cdylib` crate-type with `crate-type = ["cdylib"]` in `Cargo.toml`.
- For CI, build the example plugin and run integration tests that load it dynamically.

## Implementation Roadmap (concrete steps)

1. (This task) Add this integration plan to `docs/INTEGRATION_PLAN.md` and align stakeholders.
2. Implement the registry API and `types::registry` module with a thread-safe DashMap. Provide simple unit tests.
3. Implement loader module: `plugins::loader` using `libloading`. Add config for plugin dirs and file patterns. Add tests that load a trivial test plugin compiled in the workspace.
4. Extend `commy_macro` to generate `write_to_buffer`, `schema_text`, and `SCHEMA_HASH` for a user type. Add unit tests and example(s).
5. Wire the generated `write_to_buffer` into the SerializationBackend paths for rkyv and capnp so calls use generated writers when available.
6. Implement sample plugin crate in `examples/plugin_example` that builds as `cdylib` and registers one type.
7. Add integration tests demonstrating compiled-in type zero-copy and plugin type serialization via loader.

## Files to edit / locations

- commy_macro/ (add codegen)
- src/serialization/raw_binary.rs (hooks to detect and call registered writers)
- src/serialization/capnproto_backend.rs (use generated schema_text and replace serde bridge)
- src/plugins/loader.rs (new)
- src/types/registry.rs (new)
- examples/plugin_example/ (new cdylib plugin crate)
- docs/INTEGRATION_PLAN.md (this file)

## Acceptance Criteria

- Integration plan document exists in `docs/INTEGRATION_PLAN.md`.
- Tests demonstrating generated writer zero-copy into mmapped region pass locally.
- Plugin example can be built for host platform and loaded by the host example to perform serialization.

## Next Steps

- Implement `types::registry` and `plugins::loader` (ID 2 in the repo todo list). Mark this doc as complete and begin the proc-macro implementation.

---
Document created by the Commy integration planning process.
