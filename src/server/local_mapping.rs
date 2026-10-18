//! Server side of the local-mapping handshake (plan C1).
//!
//! The server never sees stores into a mapping, so a local mapping is only offered for a service an
//! admin has opened to it, and only to a caller with a write grant. The nonce proves the client
//! shares this machine's filesystem view: it is written to a small side file in the tenant
//! directory (never in the service file), read by the client, echoed over the authenticated
//! connection, and deleted on first use.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Prefix of every nonce side file, so a sweep can find and remove only these.
pub const NONCE_PREFIX: &str = ".nonce-";

/// A nonce that is neither confirmed nor stale is removed after this long.
pub const NONCE_TTL: Duration = Duration::from_secs(60);

/// Most unconfirmed nonces one session may hold. Past this a session is rate limited, so a client
/// cannot fill the tenant directory with side files.
pub const MAX_PENDING_NONCES: usize = 16;

/// A nonce handed to one session and not yet confirmed.
#[derive(Clone, Debug)]
pub struct PendingNonce {
    pub nonce: String,
    pub path: PathBuf,
}

/// Create a fresh per-connection nonce file in `tenant_dir`. Mode 0600 on Unix; on Windows the file
/// inherits the directory's access control list.
pub fn create_nonce_file(tenant_dir: &Path) -> std::io::Result<PendingNonce> {
    // Two v4 UUIDs: 244 random bits from the operating system's generator.
    let nonce = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let path = tenant_dir.join(format!("{NONCE_PREFIX}{}", uuid::Uuid::new_v4().simple()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    file.write_all(nonce.as_bytes())?;
    file.sync_all()?;
    Ok(PendingNonce { nonce, path })
}

/// Remove nonce side files in `dir` older than `ttl`. Other files are never touched.
pub fn sweep_stale(dir: &Path, ttl: Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let is_nonce = entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(NONCE_PREFIX));
        if !is_nonce {
            continue;
        }
        let age = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok());
        if age.is_some_and(|a| a >= ttl) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Compare two secrets without stopping at the first differing byte.
pub fn secrets_match(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    diff == 0
}

/// An absolute form of `p` that does not resolve symlinks and has no `\\?\` prefix.
pub fn absolute(p: &Path) -> std::io::Result<PathBuf> {
    std::path::absolute(p)
}
