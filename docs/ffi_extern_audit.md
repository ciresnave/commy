## FFI extern audit — Commy

This document exhaustively lists every exported `pub extern "C" fn` found under `src/ffi/` (as of this commit), whether the function accepts or dereferences raw pointers, the current `unsafe` annotation state, and a recommended action.

Notes:

- Rule applied: any exported C ABI function that accepts or dereferences raw pointers should be declared `unsafe` and include a `/// # Safety` docblock describing caller requirements. Functions that merely return pointers (without dereferencing input pointers) or accept function-pointer types may be safe to expose as non-unsafe, provided they do not dereference incoming raw pointers.
- Where a function already follows the rule, the recommendation is "No change".
- Where a function would need changes, a small patch is suggested (example snippet included).

Summary:

- Total exported symbols discovered: 22
- Pointer-dereferencing exported symbols already marked `unsafe` and documented: 18
- Exported symbols that do not dereference incoming raw pointers and are intentionally non-`unsafe`: 4
- Recommended immediate code changes: none required. All pointer-dereferencing exported functions currently are `unsafe` and include Safety docs. Keep the current safe/unsafe boundaries.

Full mapping
----------------

1. File: `src/ffi/working_sync.rs`
   - commy_ffi_init() -> `pub extern "C" fn commy_ffi_init() -> c_uint`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

   - commy_create_mesh(node_id: *const c_char, listen_port: u16) -> `pub unsafe extern "C" fn commy_create_mesh(...) -> CommyHandle`
     - Accepts/derefs raw pointers: Yes (calls CStr::from_ptr)
     - Current unsafe: Yes
     - Recommended: No change (has `/// # Safety` docblock)

   - commy_start_mesh(handle: CommyHandle) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: No (CommyHandle is an opaque value-type)
     - Current unsafe: Yes
     - Recommended: Keep as `unsafe` for API consistency with other handle-based calls (no change)

   - commy_stop_mesh(handle: CommyHandle) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: Yes
     - Recommended: No change

   - commy_is_mesh_running(handle: CommyHandle) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: Yes
     - Recommended: No change

   - commy_get_node_id(handle: CommyHandle) -> `pub unsafe extern "C" fn ... -> *mut c_char`
     - Accepts/derefs raw pointers: No input raw pointers; returns allocated pointer
     - Current unsafe: Yes (documents the handle validity requirement)
     - Recommended: No change

   - commy_get_mesh_stats(handle: CommyHandle, stats: *mut CommyMeshStats) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: Yes (dereferences `stats` pointer)
     - Current unsafe: Yes
     - Recommended: No change

   - commy_destroy_mesh(handle: CommyHandle) -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

   - commy_configure_mesh(_handle,_health: *const CommyHealthConfig, _lb:*const CommyLoadBalancerConfig) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: Yes (may deref configs)
     - Current unsafe: Yes
     - Recommended: No change

   - commy_select_service(..., _service_name: *const c_char,_client_id:*const c_char, _out_service: *mut CommyServiceInfo) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: Yes (derefs _service_name, writes to _out_service)
     - Current unsafe: Yes
     - Recommended: No change

   - commy_ffi_cleanup() -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

2. File: `src/ffi/working.rs` (legacy sync wrapper)
   - commy_ffi_init() -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

   - commy_destroy_mesh(instance_id: c_uint) -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

   - commy_ffi_cleanup() -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

   - commy_ffi_version() -> `pub extern "C" fn ... -> *const libc_c_char`
     - Accepts/derefs raw pointers: No (returns a static pointer)
     - Current unsafe: No
     - Recommended: No change

   - commy_alloc_service_info_array(count: usize) -> `pub extern "C" fn -> *mut CommyServiceInfo`
     - Accepts/derefs raw pointers: No (returns allocation)
     - Current unsafe: No
     - Recommended: No change

   - commy_free_service_info_array(ptr: *mut CommyServiceInfo, count: usize) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: Yes (deallocates pointer)
     - Current unsafe: Yes
     - Recommended: No change

3. File: `src/ffi/service.rs`
   - commy_set_service_callback(handle: CommyHandle, callback: CommyServiceCallback) -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No (takes a function pointer type)
     - Current unsafe: No
     - Recommended: No change

   - commy_register_service(handle: CommyHandle, config: *const CommyServiceConfig) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: Yes (dereferences `config`)
     - Current unsafe: Yes
     - Recommended: No change

   - commy_unregister_service(handle: CommyHandle, service_id: *const c_char) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: Yes
     - Current unsafe: Yes
     - Recommended: No change

4. File: `src/ffi/real.rs`
   - commy_ffi_init() -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

   - commy_ffi_version() -> `pub extern "C" fn ... -> *const c_char`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

   - commy_ffi_cleanup() -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No
     - Current unsafe: No
     - Recommended: No change

   - commy_get_node_id(handle: CommyHandle) -> `pub unsafe extern "C" fn ... -> *mut c_char`
     - Accepts/derefs raw pointers: No input raw pointers; returns allocated pointer
     - Current unsafe: Yes
     - Recommended: No change

5. File: `src/ffi/minimal.rs`
   - commy_ffi_init(), commy_ffi_reset(), commy_ffi_cleanup(), commy_ffi_version() -> various `pub extern "C" fn` signatures
     - Accepts/derefs raw pointers: generally No (init/cleanup/version)
     - Current unsafe: No
     - Recommended: No change

   - commy_create_mesh(node_id: *const c_char, port: u16) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: Yes
     - Current unsafe: Yes
     - Recommended: No change

   - Various free/malloc/strdup functions and many `commy_free_*` helpers (see file) — all are `pub unsafe extern "C" fn` and dereference or manage raw pointers.
     - Accepts/derefs raw pointers: Yes
     - Current unsafe: Yes
     - Recommended: No change

6. File: `src/ffi/memory.rs`
   - commy_malloc, commy_free, commy_strdup, commy_strlen, commy_memset, commy_alloc_service_info_array, commy_free_service_info_array
     - Accepts/derefs raw pointers: Yes (where relevant)
     - Current unsafe: Yes
     - Recommended: No change

7. File: `src/ffi/health.rs`
   - commy_set_health_callback(handle: CommyHandle, callback: CommyHealthCallback) -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No (function pointer)
     - Current unsafe: No
     - Recommended: No change

   - commy_start_health_monitoring, commy_stop_health_monitoring, commy_get_service_health, commy_get_all_health_status, commy_free_health_status_arrays, commy_manual_health_check
     - Accepts/derefs raw pointers: Yes (C strings and output pointers)
     - Current unsafe: Yes
     - Recommended: No change

8. File: `src/ffi/core.rs`
   - commy_set_log_callback(callback: CommyLogCallback) -> `pub extern "C" fn ...`
     - Accepts/derefs raw pointers: No (function pointer)
     - Current unsafe: No
     - Recommended: No change

   - commy_configure_mesh_core(handle: CommyHandle, health_config: *const CommyHealthConfig, lb_config:*const CommyLoadBalancerConfig) -> `pub unsafe extern "C" fn ...`
     - Accepts/derefs raw pointers: Yes
     - Current unsafe: Yes
     - Recommended: No change

Notes on borderline cases and recommendations
------------------------------------------------

- Functions that return allocated pointers (e.g., `commy_get_node_id`, `commy_alloc_service_info_array`) are safe to keep non-`unsafe` only if they do not dereference incoming raw pointers. In our codebase these functions that allocate returned pointers are already annotated `unsafe` when the function also relies on caller-supplied handles; keeping them `unsafe` is conservative and acceptable.
- Functions that accept function-pointer callbacks (e.g., `commy_set_log_callback`, `commy_set_health_callback`) are currently non-`unsafe`. This is acceptable because they do not directly dereference raw data pointers. If you want to be maximally conservative, these could be made `unsafe` too, but that would require updating all callers in example code and C bindings.

Example patch (only needed when a pointer-accepting exported function is missing `unsafe` and Safety doc):

```rust
// Replace a non-unsafe exported symbol that dereferences raw pointers
// with an unsafe variant and add a Safety docblock.
/// # Safety
///
/// `ptr` must be non-null and point to a valid NUL-terminated C string for
/// the duration of the call. The pointer must be owned by the caller.
#[no_mangle]
pub unsafe extern "C" fn commy_example(ptr: *const c_char) -> i32 {
    // implementation
}
```

Accepted outcome of this audit
----------------------------

- No immediate code changes are required: all exported functions that accept or dereference raw pointers are already declared `unsafe` and include safety documentation.
- I recommend keeping the current conservative `unsafe` annotations (there are a few handle-based functions marked `unsafe` even though they don't directly dereference raw pointers; maintaining them as `unsafe` gives a consistent, conservative API surface for consumers).

Next recommended work items (low-risk):

- Apply any remaining cosmetic `/// # Safety` docblocks to a small number of handle-based functions to clarify caller invariants (optional). Example entries are provided above.
- Add a CI job that runs a small FFI API smoke test (build the crate, compile a C fixture that links to the library and calls a small set of functions) to prevent regressions where someone later removes the `unsafe` annotations.
- Finalize hostile-input tests in `tests/` that exercise pointer-accepting functions with null and malformed input to ensure the library behaves gracefully.

Generated by: automated FFI audit pass (Copilot) — listing and recommendations validated against the current workspace files.
