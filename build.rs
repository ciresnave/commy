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
    // Emit some diagnostics about the environment so CI logs contain useful
    // context if the plugin crashes. Print PATH and candidate locations we
    // expect capnpc to be found at on Windows so maintainers can triage.
    if cfg!(windows) {
        if let Ok(path) = std::env::var("PATH") {
            println!("cargo:warning=PATH={}", path);
        }
        let user_bin = std::env::var("USERPROFILE")
            .ok()
            .map(|p| format!("{}\\.cargo\\bin", p));
        println!(
            "cargo:warning=capnpc candidates: {:?}",
            [
                user_bin.as_deref(),
                Some("C:\\Program Files\\capnproto\\bin\\capnpc.exe")
            ]
        );
    }

    // Prefer full backtraces from the plugin to aid debugging when it panics.
    // capnpc is a separate process; ensure RUST_BACKTRACE=full is present in
    // its environment so the plugin prints a verbose backtrace to stderr.
    let mut cmd = capnpc::CompilerCommand::new();
    cmd.output_path(&out_dir);
    for capnp_file in &capnp_files {
        cmd.file(capnp_file);
    }

    // Inject RUST_BACKTRACE=full into the child's environment when possible.
    // capnpc::CompilerCommand currently exposes a way to set env via
    // `env` method on CommandBuilder; fall back to OS env if not available.
    #[allow(unused_mut)]
    {
        // This mirrors the env var for the current process to ensure child
        // processes print full backtraces. It's safe to set even if already
        // present.
        std::env::set_var("RUST_BACKTRACE", "full");
    }

    // Retry the combined invocation a few times to avoid flakiness caused by
    // transient IO or process issues. If it still fails, capture diagnostics
    // (error messages and plugin probes) into files under OUT_DIR so CI
    // artifacts contain them for post-mortem analysis.
    let max_attempts = 3u32;
    let mut last_err: Option<String> = None;
    let mut success = false;
    for attempt in 1..=max_attempts {
        match cmd.run() {
            Ok(_) => {
                success = true;
                break;
            }
            Err(e) => {
                let emsg = format!("{}", e);
                println!(
                    "cargo:warning=capnpc attempt {}/{} failed: {}",
                    attempt, max_attempts, emsg
                );
                last_err = Some(emsg.clone());
                if attempt < max_attempts {
                    // small backoff
                    std::thread::sleep(std::time::Duration::from_millis(200 * attempt as u64));
                }
            }
        }
    }

    if success {
        // The capnp compiler may emit generated files into subdirectories that
        // mirror the source path (for example `OUT_DIR/schemas/...`). Move
        // any `*_capnp.rs` files found recursively into the OUT_DIR root so
        // downstream `include!`s are stable.
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

        // Emit a check-cfg directive to appease `check-cfg` warnings and
        // confirm codegen availability to downstream crates.
        println!("cargo:rustc-cfg=capnp_generated");
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
                println!(
                    "cargo:warning=No normalized *_capnp.rs files present in OUT_DIR after capnp codegen and normalization.\n\
                    This is likely caused by capnpc emitting files into an unexpected subdirectory or capnpc not being available.\n\
                    Ensure capnpc is installed and in PATH, and that build.rs has permission to move generated files.\n\
                    OUT_DIR={}",
                    out_dir.display()
                );
                panic!(
                    "capnp codegen completed but no generated Rust bindings were found in OUT_DIR"
                );
            }
        }
    } else {
        // Combined invocation failed after retries. Record the last error and
        // gather per-file diagnostics and plugin probes for investigation.
        let err_msg = last_err.unwrap_or_else(|| "unknown error".to_string());
        println!("cargo:warning=capnp codegen (combined) failed after {} attempts: {}. Attempting per-file compilation to gather diagnostics.", max_attempts, err_msg);
        // Prepare diagnostics directory
        let diag_dir = out_dir.join("capnpc_diagnostics");
        let _ = std::fs::create_dir_all(&diag_dir);
        // Write combined error to file
        let combined_path = diag_dir.join("capnpc_combined_error.txt");
        let _ = std::fs::write(&combined_path, &err_msg);
        println!(
            "cargo:warning=capnp: wrote combined error to {}",
            combined_path.display()
        );

        if err_msg.contains("PrematureEndOfFile") {
            println!("cargo:warning=capnp: detected PrematureEndOfFile from capnpc; this often indicates the plugin crashed while writing output. See diagnostics in OUT_DIR/capnpc_diagnostics");
        }

        // Probe candidate executables (version output) to capture any crashing
        // plugin's identity. These are heuristics; presence is best-effort.
        let mut probes: Vec<(String, std::process::Output)> = Vec::new();
        let mut candidates: Vec<String> = Vec::new();
        if cfg!(windows) {
            if let Some(up) = std::env::var("USERPROFILE").ok() {
                candidates.push(format!("{}\\.cargo\\bin\\capnpc-rust.exe", up));
                candidates.push(format!("{}\\.cargo\\bin\\capnpc.exe", up));
                candidates.push(format!("{}\\.cargo\\bin\\capnpc-rust-bootstrap.exe", up));
            }
            candidates.push("C:\\Program Files\\capnproto\\bin\\capnpc.exe".to_string());
        } else {
            // Common unix locations
            candidates.push("/usr/bin/capnpc".to_string());
            candidates.push("/usr/local/bin/capnpc".to_string());
            candidates.push("/usr/bin/capnpc-rust".to_string());
            candidates.push("/usr/local/bin/capnpc-rust".to_string());
        }
        for cand in candidates.iter() {
            let p = std::path::Path::new(cand);
            if p.exists() {
                match std::process::Command::new(cand).arg("--version").output() {
                    Ok(out) => {
                        probes.push((cand.clone(), out));
                    }
                    Err(_) => {
                        // best-effort; ignore probe failures
                    }
                }
            }
        }
        for (cand, out) in probes.iter() {
            // Local sanitizer: replace non-alphanumeric characters with underscores
            let fname: String = cand
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect();
            let dest = diag_dir.join(format!("{}_version.txt", fname));
            let mut combined = Vec::new();
            combined.extend_from_slice(&out.stdout);
            combined.extend_from_slice(b"\n---STDERR---\n");
            combined.extend_from_slice(&out.stderr);
            let _ = std::fs::write(&dest, &combined);
            println!(
                "cargo:warning=capnp: wrote probe output for {} -> {}",
                cand,
                dest.display()
            );
        }

        // Continue with per-file compilation attempts; capture per-file errors
        let mut per_file_results: Vec<(PathBuf, Result<(), String>)> = Vec::new();
        for capnp_file in capnp_files.iter() {
            let mut single = capnpc::CompilerCommand::new();
            single.file(capnp_file).output_path(&out_dir);
            match single.run() {
                Ok(_) => {
                    per_file_results.push((capnp_file.clone(), Ok(())));
                }
                Err(err) => {
                    let msg = format!("{}", err);
                    // write per-file diagnostic
                    if let Some(bn) = capnp_file.file_name().and_then(|s| s.to_str()) {
                        let safe = bn.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
                        let per_path = diag_dir.join(format!("{}_err.txt", safe));
                        let _ = std::fs::write(&per_path, &msg);
                        println!(
                            "cargo:warning=capnp: wrote per-file error for {} -> {}",
                            capnp_file.display(),
                            per_path.display()
                        );
                    }
                    per_file_results.push((capnp_file.clone(), Err(msg)));
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

        println!("cargo:warning=capnp codegen failed; generated bindings cleaned from OUT_DIR. Diagnostics written to: {}", diag_dir.display());
        println!("cargo:warning=To enable capnp codegen, install the `capnp` compiler (https://capnproto.org/install.html) and re-run the build with `--features capnproto`. ");
        return;
    }
}
