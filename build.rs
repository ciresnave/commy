use std::env;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

fn main() {
    // Only run capnp codegen when the `capnproto` feature is enabled.
    // Cargo exposes enabled features via environment variables of the form
    // CARGO_FEATURE_<FEATURE_NAME_IN_UPPERCASE>.
    if env::var("CARGO_FEATURE_CAPNPROTO").is_err() {
        println!("cargo:warning=capnproto feature not enabled; skipping capnp codegen");
        return;
    }

    // We will advertise the `capnp_generated` cfg to rustc's `check-cfg` machinery
    // after successful code generation below. Emitting it prematurely can hide
    // real failures; defer until we know codegen succeeded.

    // Tell cargo to rerun when any schema file changes. We will emit a
    // per-file `rerun-if-changed` below after discovering schema files.

    let out_dir = match env::var("OUT_DIR") {
        Ok(v) => PathBuf::from(v),
        Err(e) => {
            println!(
                "cargo:warning=OUT_DIR not set: {}; skipping capnp codegen",
                e
            );
            return;
        }
    };

    // Discover .capnp files recursively under schemas/ using walkdir.
    let schema_dir = PathBuf::from("schemas");
    if !schema_dir.exists() {
        println!("cargo:warning=No schemas directory found; skipping capnp codegen");
        return;
    }

    let mut capnp_files: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(&schema_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path().to_path_buf();
        if p.exists() {
            if let Ok(md) = std::fs::metadata(&p) {
                if md.is_file() && p.extension().and_then(|s| s.to_str()) == Some("capnp") {
                    capnp_files.push(p);
                }
            }
        }
    }

    if capnp_files.is_empty() {
        println!("cargo:warning=No .capnp files found under schemas/; skipping codegen");
        return;
    }

    // Emit per-file rerun-if-changed so cargo invalidates build when a schema
    // file changes.
    for f in &capnp_files {
        println!("cargo:rerun-if-changed={}", f.display());
    }

    // Invoke capnpc to generate Rust code into OUT_DIR. Be conservative: only
    // set the `capnp_generated` cfg if ALL schema files codegen successfully.
    // If any file fails, remove any partially-generated *_capnp.rs artifacts
    // to avoid exposing broken bindings to the compiler.
    // Run capnpc once for all schema files. Running the compiler in a single
    // invocation reduces the chance of partial successes and provides clearer
    // combined diagnostics if something goes wrong.
    // Single combined invocation: include the top-level schemas directory so
    // capnpc can resolve imports relative to schemas/ and produce flattened
    // output using `src_prefix` semantics when available.
    let mut cmd = capnpc::CompilerCommand::new();
    cmd.output_path(&out_dir);
    for capnp_file in &capnp_files {
        cmd.file(capnp_file);
    }

    match cmd.run() {
        Ok(_) => {
            println!(
                "cargo:warning=capnp codegen succeeded for {} file(s)",
                capnp_files.len()
            );
            // The capnp compiler may emit generated files into subdirectories
            // that mirror the source path (for example `OUT_DIR/schemas/...`).
            // Recursively search OUT_DIR for any `*_capnp.rs` files and move
            // them into the OUT_DIR root so our
            // `include!(concat!(env!("OUT_DIR"), "/..._capnp.rs"))` usages
            // find them predictably. Emit diagnostics about what we found and
            // where we moved files so CI logs show the final layout.
            // Normalize: find any *_capnp.rs files (recursively) and move them
            // into OUT_DIR root so downstream `include!`s are stable. This is
            // similar to the previous behavior but implemented with walkdir.
            let mut found: Vec<PathBuf> = Vec::new();
            for entry in WalkDir::new(&out_dir).into_iter().filter_map(|e| e.ok()) {
                let p = entry.into_path();
                if p.is_file() {
                    if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                        if fname.ends_with("_capnp.rs") {
                            found.push(p);
                        }
                    }
                }
            }

            if found.is_empty() {
                println!(
                    "cargo:warning=No generated *_capnp.rs files found in OUT_DIR after capnpc run"
                );
            } else {
                for p in &found {
                    println!("cargo:warning=found generated file: {}", p.display());
                }

                found.sort();
                found.dedup();

                for p in found {
                    if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                        let dest = out_dir.join(fname);
                        if p == dest {
                            println!(
                                "cargo:warning=skipping move; already at destination: {}",
                                p.display()
                            );
                            continue;
                        }

                        let _ = fs::remove_file(&dest);
                        match std::fs::rename(&p, &dest) {
                            Ok(()) => println!("cargo:warning=moved generated: {} -> {}", p.display(), dest.display()),
                            Err(rename_err) => match std::fs::copy(&p, &dest) {
                                Ok(_) => {
                                    let _ = fs::remove_file(&p);
                                    println!("cargo:warning=copied(moved) generated: {} -> {} (rename failed: {})", p.display(), dest.display(), rename_err);
                                }
                                Err(copy_err) => println!("cargo:warning=failed to move or copy generated file {} -> {}: rename err: {}; copy err: {}", p.display(), dest.display(), rename_err, copy_err),
                            },
                        }
                    }
                }
            }
            // Emit a check-cfg directive to appease `check-cfg` warnings.
            // Emit a cfg so consumer crates can use #[cfg(capnp_generated)] to
            // avoid hard includes when codegen was skipped. Emit exactly once.
            println!("cargo:rustc-cfg=capnp_generated");
            // Also inform Cargo's `check-cfg` machinery about this non-feature
            // cfg name so `unexpected_cfgs` diagnostics are suppressed when
            // authors use `#[cfg(capnp_generated)]`. This mirrors the advice in
            // rustc's help text and is safe to emit from the build script.
            println!("cargo:rustc-check-cfg=cfg(capnp_generated)");
            // Sanity check: ensure at least one normalized *_capnp.rs exists in OUT_DIR root.
            let normalized_exists = out_dir.join("example_capnp.rs").exists();
            if !normalized_exists {
                // If we didn't find the common example name, do a broader check.
                let mut any = false;
                if let Ok(mut entries) = std::fs::read_dir(&out_dir) {
                    while let Some(Ok(e)) = entries.next() {
                        if let Some(fname) = e.file_name().to_str() {
                            if fname.ends_with("_capnp.rs") {
                                any = true;
                                break;
                            }
                        }
                    }
                }
                if !any {
                    // Fail the build with an explanatory message so CI shows a clear error.
                    println!(
                        "cargo:warning=No normalized *_capnp.rs files present in OUT_DIR after capnp codegen and normalization.\n\
                        This is likely caused by capnpc emitting files into an unexpected subdirectory or capnpc not being available.\n\
                        Ensure capnpc is installed and in PATH, and that build.rs has permission to move generated files.\n\
                        OUT_DIR={}",
                        out_dir.display()
                    );
                    // Emit a rustc error by printing to stderr via panic! so the build fails loudly.
                    panic!("capnp codegen completed but no generated Rust bindings were found in OUT_DIR");
                }
            }
        }
        Err(e) => {
            // Combined invocation failed. Attempt per-file compilation to
            // identify which schema(s) produced errors and capture their
            // diagnostics. Then clean up any generated *_capnp.rs files.
            println!("cargo:warning=capnp codegen (combined) failed: {}. Attempting per-file compilation to gather diagnostics.", e);

            let mut per_file_results: Vec<(PathBuf, Result<(), String>)> = Vec::new();
            for capnp_file in capnp_files.iter() {
                let mut single = capnpc::CompilerCommand::new();
                single.file(capnp_file).output_path(&out_dir);
                match single.run() {
                    Ok(_) => {
                        per_file_results.push((capnp_file.clone(), Ok(())));
                    }
                    Err(err) => {
                        per_file_results.push((capnp_file.clone(), Err(format!("{}", err))));
                    }
                }
            }

            // Emit per-file diagnostics as cargo warnings to make CI logs clearer.
            for (p, res) in per_file_results.iter() {
                match res {
                    Ok(_) => println!("cargo:warning=capnp: succeeded: {}", p.display()),
                    Err(msg) => println!("cargo:warning=capnp: failed: {} -> {}", p.display(), msg),
                }
            }

            // Clean up any generated *_capnp.rs files (recursively) to avoid compiling
            // partially-generated bindings. Walk OUT_DIR one level deep.
            let _ = (|| -> std::io::Result<()> {
                for entry in fs::read_dir(&out_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        for sub in fs::read_dir(&path)? {
                            let sub = sub?;
                            let sub_path = sub.path();
                            if let Some(fname) = sub_path.file_name().and_then(|s| s.to_str()) {
                                if fname.ends_with("_capnp.rs") {
                                    let _ = fs::remove_file(&sub_path);
                                }
                            }
                        }
                    } else if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                        if fname.ends_with("_capnp.rs") {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
                Ok(())
            })();

            println!("cargo:warning=capnp codegen failed; generated bindings cleaned from OUT_DIR. To enable capnp codegen, install the `capnp` compiler (https://capnproto.org/install.html) and re-run the build with `--features capnproto`. ");
        }
    }
}
