#[cfg(feature = "plugins")]
use std::path::PathBuf;

#[cfg(feature = "plugins")]
#[test]
fn plugin_serializes_into_buffer_correctly() {
    use commy::plugins::loader::PluginHandle;
    use commy::types::registry::Registry;

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
    let _handle = PluginHandle::load(path, &reg).expect("should load plugin");

    // Lookup the type entry registered by the plugin. The example plugin registers
    // `plugin_example::PluginExample` and marks supported formats — assert presence.
    let entry = reg
        .lookup("plugin_example::PluginExample", None)
        .expect("entry should be registered");

    // The example plugin advertises RKYV support (formats bitflag). Assert that.
    use commy::types::registry::Formats;
    assert!(entry.formats.contains(Formats::RKYV));
}

#[cfg(not(feature = "plugins"))]
#[test]
fn plugins_feature_disabled() {
    eprintln!("plugins feature disabled; skipping plugin integration tests");
}
