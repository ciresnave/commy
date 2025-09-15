use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Only run capnp codegen when the `capnproto` feature is enabled.
    // Cargo exposes enabled features via environment variables of the form
    // CARGO_FEATURE_<FEATURE_NAME_IN_UPPERCASE>.
    if env::var("CARGO_FEATURE_CAPNPROTO").is_err() {
        println!("cargo:warning=capnproto feature not enabled; skipping capnp codegen");
        return;
    }

    println!("cargo:rerun-if-changed=schemas");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Copy any .capnp schema files into OUT_DIR and run capnpc on them.
    let schema_dir = PathBuf::from("schemas");
    if !schema_dir.exists() {
        println!("cargo:warning=No schemas directory found; skipping capnp codegen");
        return;
    }

    let mut capnp_files = Vec::new();
    for entry in fs::read_dir(&schema_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("capnp") {
            let filename = path.file_name().unwrap();
            let dest = out_dir.join(filename);
            fs::copy(&path, &dest).unwrap();
            capnp_files.push(dest);
        }
    }

    if capnp_files.is_empty() {
        println!("cargo:warning=No .capnp files found under schemas/; skipping codegen");
        return;
    }

    // Invoke capnpc to generate Rust code into OUT_DIR
    let mut any_generated = false;
    for capnp_file in capnp_files.iter() {
        let mut cmd = capnpc::CompilerCommand::new();
        cmd.file(capnp_file).output_path(&out_dir);
        match cmd.run() {
            Ok(_) => {
                any_generated = true;
                println!("cargo:warning=capnp codegen succeeded for {:?}", capnp_file);
            }
            Err(e) => {
                // If capnp isn't installed (capnp binary missing), the error will
                // indicate that. We should not panic the entire build in that
                // case; instead, emit a helpful warning and skip codegen so
                // developers without the native toolchain can still build the
                // crate without the `capnproto` feature.
                println!(
                    "cargo:warning=capnp codegen failed: {}. Skipping capnp codegen. To enable capnp codegen, install the `capnp` compiler (https://capnproto.org/install.html) and re-run the build with `--features capnproto`.",
                    e
                );
            }
        }
    }

    // If any codegen succeeded, expose a cfg so the Rust code can include the
    // generated bindings safely. When codegen was skipped (e.g. capnp binary
    // missing), do not set the cfg so the source can fall back to a stub.
    if any_generated {
        println!("cargo:rustc-cfg=capnp_generated");
    }
}
