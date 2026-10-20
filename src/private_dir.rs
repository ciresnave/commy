//! Tenant directories that only the Commy server and administrators can list (plan C8).
//!
//! A service file has no permissions of its own (plan Q9, deferred): a long random file name inside a
//! directory other accounts cannot *list* is the barrier. So the directory must allow other accounts
//! to pass through it (to open a file by its full path, which is how a local client maps the file)
//! but not to enumerate it.
//!
//! - Unix: mode `0711` (owner everything; everyone else may only traverse).
//! - Windows: inheritance removed; SYSTEM, Administrators and the running account get full control,
//!   inherited by what is created inside; `Users` get traverse on the directory itself and modify on
//!   the files inside (inherit-only, files only), so no account but the server and administrators
//!   can list it.
//!
//! Nothing here can be tested against a second real account on one machine; the tests check the mode
//! or the access control list, and that the owner can still use the directory.

use std::io;
use std::path::Path;

/// Create `path` (and any missing parents) and, if `path` itself did not exist, restrict it as
/// described in the module docs. An existing directory is left as the operator set it.
///
/// Fails closed: if the restriction cannot be applied the directory is not used.
pub fn create_private(path: &Path) -> io::Result<()> {
    let existed = path.exists();
    std::fs::create_dir_all(path)?;
    if !existed {
        if let Err(e) = restrict_dir(path) {
            let _ = std::fs::remove_dir(path);
            return Err(e);
        }
    }
    Ok(())
}

/// Make a file readable and writable by the server and administrators only (used for the local
/// mapping nonce, which must not be readable by other accounts even though the directory's files are).
pub fn make_file_private(path: &Path) -> io::Result<()> {
    restrict_file(path)
}

#[cfg(unix)]
fn restrict_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o711))
}

#[cfg(unix)]
fn restrict_file(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(windows)]
mod win {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    const SYSTEM: &str = "*S-1-5-18";
    const ADMINISTRATORS: &str = "*S-1-5-32-544";
    const USERS: &str = "*S-1-5-32-545";

    /// The account the server runs as, `DOMAIN\NAME`.
    fn current_account() -> io::Result<String> {
        let name = std::env::var("USERNAME")
            .map_err(|_| io::Error::other("USERNAME is not set; cannot name the server account"))?;
        Ok(match std::env::var("USERDOMAIN") {
            Ok(d) if !d.is_empty() => format!("{d}\\{name}"),
            _ => name,
        })
    }

    fn icacls(path: &Path, args: &[&str]) -> io::Result<()> {
        let out = Command::new("icacls").arg(path).args(args).output()?;
        if out.status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "icacls {:?} failed: {} {}",
                args,
                String::from_utf8_lossy(&out.stdout).trim(),
                String::from_utf8_lossy(&out.stderr).trim()
            )))
        }
    }

    pub fn restrict_dir(path: &Path) -> io::Result<()> {
        let me = current_account()?;
        let full = |who: &str| format!("{who}:(OI)(CI)F");
        icacls(
            path,
            &[
                "/inheritance:r",
                "/grant:r",
                &full(SYSTEM),
                "/grant:r",
                &full(ADMINISTRATORS),
                "/grant:r",
                &full(&me),
            ],
        )?;
        // Others may pass through the directory but not list it, and may use files inside it.
        icacls(path, &["/grant", &format!("{USERS}:(X)")])?;
        icacls(path, &["/grant", &format!("{USERS}:(OI)(IO)M")])
    }

    pub fn restrict_file(path: &Path) -> io::Result<()> {
        let me = current_account()?;
        icacls(
            path,
            &[
                "/inheritance:r",
                "/grant:r",
                &format!("{SYSTEM}:F"),
                "/grant:r",
                &format!("{ADMINISTRATORS}:F"),
                "/grant:r",
                &format!("{me}:F"),
            ],
        )
    }
}

#[cfg(windows)]
fn restrict_dir(path: &Path) -> io::Result<()> {
    win::restrict_dir(path)
}

#[cfg(windows)]
fn restrict_file(path: &Path) -> io::Result<()> {
    win::restrict_file(path)
}

#[cfg(not(any(unix, windows)))]
fn restrict_dir(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn restrict_file(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tenant_x");
        (d, p)
    }

    #[test]
    fn a_new_directory_is_created_with_its_parents() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a").join("b").join("tenant_x");
        create_private(&p).unwrap();
        assert!(p.is_dir());
    }

    #[test]
    fn the_owner_can_still_list_and_use_the_directory() {
        let (_d, p) = fresh();
        create_private(&p).unwrap();
        let f = p.join("service_1.mem");
        std::fs::write(&f, [1u8, 2, 3]).unwrap();
        let names: Vec<_> = std::fs::read_dir(&p)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
        assert_eq!(std::fs::read(&f).unwrap(), vec![1, 2, 3]);
        // and can write into an existing file by path, as the server and local clients do
        let mut rw = std::fs::OpenOptions::new().write(true).open(&f).unwrap();
        use std::io::Write;
        rw.write_all(&[9]).unwrap();
    }

    #[test]
    fn an_existing_directory_is_left_as_the_operator_set_it() {
        let (_d, p) = fresh();
        std::fs::create_dir(&p).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        create_private(&p).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o755, "an existing directory must not be re-restricted");
        }
    }

    #[cfg(unix)]
    #[test]
    fn unix_mode_lets_others_traverse_but_not_list() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, p) = fresh();
        create_private(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o711);
        assert_eq!(mode & 0o044, 0, "group and others have no read (list) bit");
        assert_eq!(mode & 0o011, 0o011, "group and others keep the search (traverse) bit");
    }

    #[cfg(unix)]
    #[test]
    fn a_private_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, p) = fresh();
        std::fs::create_dir(&p).unwrap();
        let f = p.join("n");
        std::fs::write(&f, b"x").unwrap();
        make_file_private(&f).unwrap();
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[cfg(windows)]
    fn acl(path: &Path) -> String {
        let out = std::process::Command::new("icacls").arg(path).output().unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    #[cfg(windows)]
    #[test]
    fn windows_acl_gives_users_traverse_on_the_directory_and_modify_on_files_only() {
        let (_d, p) = fresh();
        create_private(&p).unwrap();
        let text = acl(&p);
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        // administrators and system have full control, inherited
        assert!(lines.iter().any(|l| l.contains("BUILTIN\\Administrators:(OI)(CI)(F)")), "{text}");
        assert!(lines.iter().any(|l| l.contains("NT AUTHORITY\\SYSTEM:(OI)(CI)(F)")), "{text}");
        // Users: traverse on the directory itself, modify on files inside, nothing that lists
        assert!(lines.iter().any(|l| l.contains("BUILTIN\\Users:(X)")), "{text}");
        assert!(lines.iter().any(|l| l.contains("BUILTIN\\Users:(OI)(IO)(M)")), "{text}");
        for l in &lines {
            if l.contains("BUILTIN\\Users:") {
                assert!(!l.contains("(F)") && !l.contains("(RX)") && !l.contains("(R)") && !l.contains("(CI)"),
                    "Users must not be able to list the directory: {l}");
            }
            assert!(!l.contains("Everyone"), "no Everyone entry: {l}");
        }
        assert!(!text.contains("(I)"), "inheritance from the parent must be removed: {text}");
    }

    #[cfg(windows)]
    #[test]
    fn windows_private_file_has_no_users_entry() {
        // Inside a private directory a new file inherits `Users:(M)`; making it private must remove
        // that, or the nonce would be readable by every other account.
        let (_d, p) = fresh();
        create_private(&p).unwrap();
        let f = p.join("n");
        std::fs::write(&f, b"x").unwrap();
        assert!(acl(&f).contains("BUILTIN\\Users"), "precondition: the file inherits the Users grant: {}", acl(&f));
        make_file_private(&f).unwrap();
        let text = acl(&f);
        assert!(!text.contains("BUILTIN\\Users"), "{text}");
        assert!(!text.contains("Everyone"), "{text}");
        assert_eq!(std::fs::read(&f).unwrap(), b"x", "the owner still reads it");
    }

    #[test]
    fn a_file_created_in_a_private_directory_follows_the_directory_rules() {
        // On Windows the new file inherits Users:(M); on Unix it has the ordinary umask mode.
        // Either way the owner can map it.
        let (_d, p) = fresh();
        create_private(&p).unwrap();
        let f = p.join("service.mem");
        let svc = crate::Service::open_or_create(f.to_str().unwrap(), 4096).unwrap();
        assert_eq!(svc.len(), 4096);
    }
}
