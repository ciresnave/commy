// SPDX-License-Identifier: MIT
//! Writes a fresh self-signed development certificate and private key.
//!
//! Every run generates a new key pair, so no two runs (and no two users) share a key. The key
//! file is readable by its owner only. Usage: `gen_certs [OUTPUT_DIR]` (default: the current
//! directory). Existing `dev-cert.pem` / `dev-key.pem` are never overwritten.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

fn write_new(path: &Path, bytes: &[u8], private: bool) -> std::io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(bytes)?;
    #[cfg(windows)]
    if private {
        if let Err(e) = restrict_to_owner(path) {
            drop(f);
            let _ = std::fs::remove_file(path);
            return Err(e);
        }
    }
    let _ = private;
    Ok(())
}

/// Leave the current user as the only principal on the key. Dropping inherited entries is not
/// enough: a new file also gets the creating token's default DACL, which on an elevated token
/// names SYSTEM and Administrators, so those are removed explicitly by well-known SID.
#[cfg(windows)]
fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    let user = std::env::var("USERNAME")
        .map_err(|_| std::io::Error::other("USERNAME is not set; cannot restrict the key file"))?;
    let status = std::process::Command::new("icacls")
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .arg(format!("{user}:F"))
        .args(["/remove:g", "*S-1-5-18", "*S-1-5-32-544"])
        .stdout(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("icacls failed: {status}")))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args_os().nth(1).map(std::path::PathBuf::from).unwrap_or_else(|| ".".into());
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()])?;
    let cert_path = dir.join("dev-cert.pem");
    let key_path = dir.join("dev-key.pem");
    write_new(&key_path, ck.signing_key.serialize_pem().as_bytes(), true)?;
    if let Err(e) = write_new(&cert_path, ck.cert.pem().as_bytes(), false) {
        let _ = std::fs::remove_file(&key_path);
        return Err(e.into());
    }

    println!("Generated a new self-signed certificate and key (SAN: localhost, 127.0.0.1):");
    println!("  - {} (certificate)", cert_path.display());
    println!("  - {} (private key, owner-only; do not share or commit it)", key_path.display());
    println!("Point the server at them with COMMY_TLS_CERT_PATH and COMMY_TLS_KEY_PATH.");
    Ok(())
}
