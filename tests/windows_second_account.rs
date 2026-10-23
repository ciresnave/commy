//! R9: the tenant-directory access control list against a second real Windows account.
//!
//! What `private_dir` promises (and `docs` and the CHANGELOG repeat): another account can open a file
//! in a tenant directory by its full path but cannot list the directory; a `create_secret` directory
//! cannot even be passed through; a `make_file_private` file cannot be read. Until now only the ACL
//! text and the owner's access were checked on Windows.
//!
//! The second account is AntSpawn's restricted `antspawn-run`, reached through AntSpawn's `account`
//! launcher (`spawn run ... --launcher account`): the scheduled task `AntSpawnRunner` runs the job AS
//! that account. The test never creates an account, never handles a password and never elevates.
//!
//! Needs, and prints `UNVERIFIED` and returns without them (never `#[ignore]`d):
//!   * Windows, with the task `AntSpawnRunner` registered (AntSpawn's one-time elevated setup);
//!   * `COMMY_R9_SPAWN`, the path of a built `spawn.exe`, and `COMMY_R9_PROBE`, the path of the
//!     `spawn-probe.exe` built beside it (AntSpawn `cargo build --bin spawn --bin spawn-probe`).
//!
//! Files are opened with AntSpawn's `spawn-probe read`, a plain `CreateFile`, because that is what a
//! local client does. `cmd /c type` is NOT used for this: it resolves the name through `FindFirstFile`,
//! which lists the parent directory, and listing is exactly what the tenant directory refuses.
//!
//! The directories are made OUTSIDE `C:\Projects` on purpose: the account setup denies `antspawn-run`
//! all of `C:\Projects`, and a denial from that deny would say nothing about Commy's ACL. The
//! `control` directory (default ACLs, same parent) must be listable and readable by the account, which
//! proves the parent is reachable, so each refusal below comes from the ACL under test.
//!
//! "Denied for the right reason": a refused job must also leave output that does not say the path was
//! not found. The words are English (`cannot find`); on another locale that check is weaker, and the
//! control still shows the path exists.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

use commy::private_dir::{create_private, create_secret, make_file_private};

const TASK: &str = "AntSpawnRunner";

fn spawn_exe() -> Option<(PathBuf, PathBuf)> {
    let Some(probe) = std::env::var_os("COMMY_R9_PROBE").map(PathBuf::from) else {
        println!("UNVERIFIED: COMMY_R9_PROBE is not set (path of AntSpawn's spawn-probe.exe)");
        return None;
    };
    if !probe.is_file() {
        println!("UNVERIFIED: COMMY_R9_PROBE does not name a file: {}", probe.display());
        return None;
    }
    let Some(p) = std::env::var_os("COMMY_R9_SPAWN").map(PathBuf::from) else {
        println!("UNVERIFIED: COMMY_R9_SPAWN is not set (path of a built AntSpawn spawn.exe)");
        return None;
    };
    let task_ok = Command::new("schtasks")
        .args(["/Query", "/TN", TASK])
        .output()
        .is_ok_and(|o| o.status.success());
    if !p.is_file() {
        println!("UNVERIFIED: COMMY_R9_SPAWN does not name a file: {}", p.display());
        return None;
    }
    if !task_ok {
        println!("UNVERIFIED: task {TASK} is not registered; AntSpawn's account setup has not been applied here");
        return None;
    }
    Some((p, probe))
}

/// A base directory under `C:\ProgramData`, removed on drop. Never under `C:\Projects`.
struct Base(PathBuf);

impl Base {
    fn new() -> Self {
        let root = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        let p = root.join(format!("commy-r9-{}", std::process::id()));
        let lower = p.to_string_lossy().to_ascii_lowercase();
        assert!(
            !lower.starts_with(r"c:\projects"),
            "the test directory must be outside C:\\Projects, where the account is denied anyway: {}",
            p.display()
        );
        assert!(
            !p.to_string_lossy().contains(' '),
            "the job's command line is not quoted; use a path without spaces: {}",
            p.display()
        );
        std::fs::create_dir_all(&p).unwrap();
        Base(p)
    }
}

impl Drop for Base {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a job as `antspawn-run` did: its exit code and what it wrote to its `out.txt`.
struct Ran {
    code: i32,
    out: String,
}

/// Run `cmd.exe /c <action> <path> > out.txt 2>&1` as `antspawn-run`.
fn as_account(spawn: &Path, work: &Path, name: &str, action: &str, path: &Path) -> Ran {
    let res = work.join(format!("res-{name}"));
    let spec = work.join(format!("{name}.toml"));
    let line = format!("{action} {} > out.txt 2>&1", path.display());
    std::fs::write(
        &spec,
        format!(
            "[job]\nname = \"{name}\"\ncmd = ['cmd.exe', '/c', '{line}']\n[limits]\ntime_s = 30\n\
             [[output]]\nguest = \"out.txt\"\nname = \"out\"\n"
        ),
    )
    .unwrap();
    let status = Command::new(spawn)
        .args(["run"])
        .arg(&spec)
        .args(["--launcher", "account", "--results"])
        .arg(&res)
        .status()
        .expect("spawn.exe could not be started");
    let code = status.code().unwrap_or(-1);
    assert_ne!(
        code, 2,
        "spawn itself failed (exit 2), not the job: the setup is not what this test assumes"
    );
    let out = std::fs::read_to_string(res.join("out")).unwrap_or_default();
    Ran { code, out }
}

/// Open `path` as `antspawn-run` with `spawn-probe read` (a plain open; exit 0 if it could be read).
fn read_as_account(spawn: &Path, probe: &Path, work: &Path, name: &str, path: &Path) -> Ran {
    let res = work.join(format!("res-{name}"));
    let spec = work.join(format!("{name}.toml"));
    std::fs::write(
        &spec,
        format!(
            "[job]
name = \"{name}\"
cmd = ['spawn-probe.exe', 'read', '{}']
[limits]
time_s = 30
             [[mount]]
host = '{}'
guest = \"spawn-probe.exe\"
mode = \"ro\"
",
            path.display(),
            probe.display()
        ),
    )
    .unwrap();
    let status = Command::new(spawn)
        .args(["run"])
        .arg(&spec)
        .args(["--launcher", "account", "--results"])
        .arg(&res)
        .status()
        .expect("spawn.exe could not be started");
    let code = status.code().unwrap_or(-1);
    assert_ne!(code, 2, "spawn itself failed (exit 2), not the job");
    Ran { code, out: String::new() }
}

fn assert_allowed(r: &Ran, what: &str) {
    assert_eq!(r.code, 0, "{what}: the account was refused, output: {}", r.out);
}

fn assert_refused(r: &Ran, what: &str) {
    assert_ne!(r.code, 0, "{what}: the account was NOT refused, output: {}", r.out);
    assert!(
        !r.out.to_ascii_lowercase().contains("cannot find"),
        "{what}: refused because the path was not found, not because of the ACL: {}",
        r.out
    );
}

#[test]
fn a_second_real_account_can_open_a_tenant_file_but_not_list_or_pass_secret_state() {
    let Some((spawn, probe)) = spawn_exe() else {
        return;
    };
    let base = Base::new();
    let b = &base.0;

    // Control: default ACLs. The account must be able to list and read here; this is the known-bad
    // case that proves the parent is reachable and the probes can succeed.
    let control = b.join("control");
    std::fs::create_dir_all(&control).unwrap();
    std::fs::write(control.join("f.txt"), b"control").unwrap();

    // A tenant directory as the server makes it, with a file inside.
    let tenant = b.join("tenant");
    create_private(&tenant).unwrap();
    std::fs::write(tenant.join("svc.bin"), b"service bytes").unwrap();
    // The same directory also holds a nonce file made private.
    std::fs::write(tenant.join("nonce"), b"nonce").unwrap();
    make_file_private(&tenant.join("nonce")).unwrap();

    // State that must not even be traversed.
    let secret = b.join("secret");
    create_secret(&secret).unwrap();
    std::fs::write(secret.join("grants"), b"grants").unwrap();

    // The owner (this account) keeps full use of everything, so a refusal is not the owner's own.
    assert_eq!(std::fs::read(tenant.join("svc.bin")).unwrap(), b"service bytes");
    assert_eq!(std::fs::read(tenant.join("nonce")).unwrap(), b"nonce");
    assert_eq!(std::fs::read(secret.join("grants")).unwrap(), b"grants");
    assert!(std::fs::read_dir(&tenant).unwrap().count() >= 2);

    // Controls first: the account can list and read the default-ACL directory.
    assert_allowed(&as_account(&spawn, b, "list-control", "dir", &control), "list control");
    assert_allowed(&read_as_account(&spawn, &probe, b, "read-control", &control.join("f.txt")), "read control");

    // The promise: open a file in a tenant directory by its full path ...
    assert_allowed(
        &read_as_account(&spawn, &probe, b, "read-tenant-file", &tenant.join("svc.bin")),
        "read svc.bin in the tenant directory by its full path",
    );
    // ... but not list the directory,
    assert_refused(&as_account(&spawn, b, "list-tenant", "dir", &tenant), "list the tenant directory");
    // not read the private nonce file,
    assert_refused(
        &read_as_account(&spawn, &probe, b, "read-nonce", &tenant.join("nonce")),
        "read the private nonce file",
    );
    // and not pass through the secret directory at all.
    assert_refused(
        &read_as_account(&spawn, &probe, b, "read-secret", &secret.join("grants")),
        "read a file inside a create_secret directory",
    );
    assert_refused(&as_account(&spawn, b, "list-secret", "dir", &secret), "list the secret directory");
}
