#[cfg(feature = "plugins")]
#[test]
fn plugin_ffi_edge_cases() {
    use commy::plugins::loader::PluginHandle;
    use commy::types::registry::Registry;
    use std::ffi::CStr;
    use std::path::PathBuf;

    // Locate built plugin artifact
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("examples/plugin_example/target/debug");

    #[cfg(target_os = "windows")]
    path.push("plugin_example.dll");
    #[cfg(target_os = "linux")]
    path.push("libplugin_example.so");
    #[cfg(target_os = "macos")]
    path.push("libplugin_example.dylib");

    if !path.exists() {
        eprintln!("Plugin not built at {:?}, skipping test. Run `cargo build --manifest-path examples/plugin_example/Cargo.toml` before running tests.", path);
        return;
    }

    let reg = Registry::new();
    // Keep a copy of the path so we can open the library directly for ABI inspection.
    let lib_path = path.clone();
    let _handle = PluginHandle::load(path.clone(), &reg).expect("should load plugin");

    // Use libloading to get the descriptor symbol and inspect descriptors directly.
    use libloading::Library;
    let lib = unsafe { Library::new(lib_path).expect("open lib") };

    // If we can get the symbol, call it and validate behaviors
    unsafe {
        type GetDesc =
            unsafe extern "C" fn(
                *mut usize,
            )
                -> *const *const commy::plugins::loader::PluginTypeDescriptor;
        if let Ok(sym) = lib.get::<GetDesc>(b"com_my_plugin_get_descriptors\0") {
            let get_desc: GetDesc = *sym;
            let mut n: usize = 0;
            let arr = get_desc(&mut n as *mut usize);
            assert!(!arr.is_null());
            assert!(n >= 1);
            let slice = std::slice::from_raw_parts(arr, n);
            let first = slice[0];
            assert!(!first.is_null());
            let desc = &*first;

            // Validate type name C string is valid
            let cstr = CStr::from_ptr(desc.type_name);
            let s = cstr.to_string_lossy();
            assert!(s.contains("plugin_example::PluginExample"));

            // get_schema_text should return a pointer
            if let Some(get_schema) = desc.get_schema_text {
                let p = get_schema(desc.ctx);
                assert!(!p.is_null());
                let txt = CStr::from_ptr(p).to_string_lossy();
                assert!(txt.len() > 0);
            }

            // serialize_into should handle null pointers safely (our plugin returns 0)
            if let Some(ser) = desc.serialize_into {
                let zero = ser(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    0,
                );
                assert_eq!(zero, 0usize);
            }
        }
    }
}

#[cfg(not(feature = "plugins"))]
#[test]
fn plugins_feature_disabled() {
    eprintln!("plugins feature disabled; skipping plugin FFI edge tests");
}
