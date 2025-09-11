#[cfg(feature = "plugins")]
use std::path::PathBuf;

#[cfg(feature = "plugins")]
#[test]
fn load_example_plugin_if_present() {
    use commy::plugins::loader::PluginHandle;
    use commy::types::registry::Registry;

    // Locate example plugin build output
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
    let _handle = PluginHandle::load(path, &reg).expect("should load plugin");

    // We don't assert on registration details here; the plugin may be minimal.
}

#[cfg(not(feature = "plugins"))]
#[test]
fn plugins_feature_disabled() {
    eprintln!("plugins feature disabled; skipping plugin loader tests");
}
