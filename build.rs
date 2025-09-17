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

    // Always advertise the `capnp_generated` cfg to rustc's `check-cfg` machinery so
    // code that contains `#[cfg(capnp_generated)]` does not trigger the
    // `unexpected_cfgs` lint when the build script runs before generation.
    println!("cargo:rustc-check-cfg=cfg(capnp_generated)");

    println!("cargo:rerun-if-changed=schemas");

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

    // Copy any .capnp schema files into OUT_DIR and run capnpc on them.
    let schema_dir = PathBuf::from("schemas");
    if !schema_dir.exists() {
        println!("cargo:warning=No schemas directory found; skipping capnp codegen");
        return;
    }

    let mut capnp_files = Vec::new();
    use std::collections::HashSet;
    let mut seen_basenames: HashSet<String> = HashSet::new();
    let read_dir = match fs::read_dir(&schema_dir) {
        Ok(rd) => rd,
        Err(e) => {
            println!(
                "cargo:warning=Failed to read schemas directory: {}; skipping capnp codegen",
                e
            );
            return;
        }
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                println!(
                    "cargo:warning=Failed to read entry in schemas directory: {}; skipping entry",
                    e
                );
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("capnp") {
            let filename = match path
                .file_name()
                .and_then(|f| f.to_str().map(|s| s.to_string()))
            {
                Some(f) => f,
                None => continue,
            };
            let basename = filename.clone();
            if !seen_basenames.insert(basename.clone()) {
                println!("cargo:warning=Duplicate schema basename detected: {}. Generated artifacts may collide in OUT_DIR", basename);
            }
            let dest = out_dir.join(&filename);
            match fs::copy(&path, &dest) {
                Ok(_) => {
                    // Emit diagnostics about the copied schema to help CI debug
                    // cases where a schema may be truncated or corrupted.
                    match fs::metadata(&dest) {
                        Ok(meta) => {
                            let size = meta.len();
                            println!(
                                "cargo:warning=copied schema: {} -> {} ({} bytes)",
                                path.display(),
                                dest.display(),
                                size
                            );
                            // Try to show a short preview of the file (first 512 bytes)
                            if let Ok(mut f) = fs::File::open(&dest) {
                                use std::io::Read;
                                let mut buf = [0u8; 512];
                                if let Ok(n) = f.read(&mut buf) {
                                    if n > 0 {
                                        // Print as UTF-8 lossily to avoid panics on binary data.
                                        let preview = String::from_utf8_lossy(&buf[..n]);
                                        for line in preview.lines().take(20) {
                                            println!("cargo:warning=schema-preview: {}", line);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => println!(
                            "cargo:warning=Failed to stat copied schema {}: {}",
                            dest.display(),
                            e
                        ),
                    }

                    capnp_files.push(dest)
                }
                Err(e) => println!(
                    "cargo:warning=Failed to copy {:?} to {:?}: {}",
                    path, dest, e
                ),
            }
        }
    }

    if capnp_files.is_empty() {
        println!("cargo:warning=No .capnp files found under schemas/; skipping codegen");
        return;
    }

    // Invoke capnpc to generate Rust code into OUT_DIR. Be conservative: only
    // set the `capnp_generated` cfg if ALL schema files codegen successfully.
    // If any file fails, remove any partially-generated *_capnp.rs artifacts
    // to avoid exposing broken bindings to the compiler.
    // Run capnpc once for all schema files. Running the compiler in a single
    // invocation reduces the chance of partial successes and provides clearer
    // combined diagnostics if something goes wrong.
    let mut cmd = capnpc::CompilerCommand::new();
    for capnp_file in capnp_files.iter() {
        cmd.file(capnp_file);
    }
    cmd.output_path(&out_dir);

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
            if let Err(e) = (|| -> std::io::Result<()> {
                // Collect matching files recursively.
                fn collect(
                    dir: &std::path::Path,
                    acc: &mut Vec<std::path::PathBuf>,
                ) -> std::io::Result<()> {
                    for entry in std::fs::read_dir(dir)? {
                        let entry = entry?;
                        let p = entry.path();
                        if p.is_dir() {
                            collect(&p, acc)?;
                        } else if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                            if fname.ends_with("_capnp.rs") {
                                acc.push(p);
                            }
                        }
                    }
                    Ok(())
                }

                let mut found: Vec<std::path::PathBuf> = Vec::new();
                collect(&out_dir, &mut found)?;

                if found.is_empty() {
                    println!("cargo:warning=No generated *_capnp.rs files found in OUT_DIR after capnpc run");
                } else {
                    for p in &found {
                        println!("cargo:warning=found generated file: {}", p.display());
                    }

                    // Deduplicate paths to avoid double-processing the same file.
                    found.sort();
                    found.dedup();

                    for p in found {
                        if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                            let dest = out_dir.join(fname);

                            // If source and destination are identical, skip moving.
                            if p == dest {
                                println!(
                                    "cargo:warning=skipping move; already at destination: {}",
                                    p.display()
                                );
                                continue;
                            }

                            // If dest exists, remove it first to allow overwrite.
                            let _ = std::fs::remove_file(&dest);

                            // Prefer atomic rename; on Windows or some platforms this
                            // can fail across device boundaries, so fall back to copy
                            // + remove if rename fails.
                            match std::fs::rename(&p, &dest) {
                                Ok(()) => {
                                    println!(
                                        "cargo:warning=moved generated: {} -> {}",
                                        p.display(),
                                        dest.display()
                                    );
                                }
                                Err(rename_err) => {
                                    // Fallback: try copy then remove source.
                                    match std::fs::copy(&p, &dest) {
                                        Ok(_) => {
                                            let _ = std::fs::remove_file(&p);
                                            println!(
                                                "cargo:warning=copied(moved) generated: {} -> {} (rename failed: {})",
                                                p.display(),
                                                dest.display(),
                                                rename_err
                                            );
                                        }
                                        Err(copy_err) => {
                                            return Err(std::io::Error::new(
                                                copy_err.kind(),
                                                format!(
                                                    "failed to move or copy generated file {} -> {}: rename err: {}; copy err: {}",
                                                    p.display(), dest.display(), rename_err, copy_err
                                                ),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(())
            })() {
                println!(
                    "cargo:warning=Failed to normalize/generated capnp outputs recursively: {}",
                    e
                );
            }
            // Emit a check-cfg directive to appease `check-cfg` warnings.
            println!("cargo:rustc-check-cfg=cfg(capnp_generated)");
            println!("cargo:rustc-cfg=capnp_generated");
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
