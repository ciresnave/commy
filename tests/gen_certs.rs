// SPDX-License-Identifier: MIT
//! `gen_certs` must make a new key every run and keep the key file private.

use std::path::Path;
use std::process::Command;

fn run(dir: &Path) {
    let out = Command::new(env!("CARGO_BIN_EXE_gen_certs"))
        .arg(dir)
        .output()
        .expect("run gen_certs");
    assert!(
        out.status.success(),
        "gen_certs failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn two_runs_make_different_keys_and_certificates() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    run(a.path());
    run(b.path());
    let key_a = std::fs::read_to_string(a.path().join("dev-key.pem")).unwrap();
    let key_b = std::fs::read_to_string(b.path().join("dev-key.pem")).unwrap();
    let cert_a = std::fs::read_to_string(a.path().join("dev-cert.pem")).unwrap();
    assert!(key_a.contains("PRIVATE KEY"), "not a PEM private key");
    assert!(cert_a.contains("BEGIN CERTIFICATE"));
    assert_ne!(key_a, key_b, "two runs wrote the same private key");
    assert_ne!(cert_a, std::fs::read_to_string(b.path().join("dev-cert.pem")).unwrap());
}

#[test]
fn the_output_loads_as_a_tls_server_identity() {
    let dir = tempfile::tempdir().unwrap();
    run(dir.path());
    commy::server::tls::TlsConfiguration::from_files(
        dir.path().join("dev-cert.pem").to_str().unwrap(),
        dir.path().join("dev-key.pem").to_str().unwrap(),
    )
    .expect("the generated pair is a usable TLS identity");
}

#[test]
fn an_existing_key_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    run(dir.path());
    let before = std::fs::read_to_string(dir.path().join("dev-key.pem")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_gen_certs")).arg(dir.path()).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(before, std::fs::read_to_string(dir.path().join("dev-key.pem")).unwrap());
}

#[cfg(unix)]
#[test]
fn the_key_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    run(dir.path());
    let mode = std::fs::metadata(dir.path().join("dev-key.pem")).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "group/other can access the key: {mode:o}");
}

#[cfg(windows)]
#[test]
fn the_key_file_acl_names_only_the_owner() {
    let dir = tempfile::tempdir().unwrap();
    run(dir.path());
    let out = Command::new("icacls").arg(dir.path().join("dev-key.pem")).output().unwrap();
    let acl = String::from_utf8_lossy(&out.stdout).to_string();
    let user = std::env::var("USERNAME").unwrap();
    assert!(acl.contains(&user), "owner missing from ACL: {acl}");
    for other in ["Everyone", r"BUILTIN\Users", "Authenticated Users", r"BUILTIN\Administrators", r"NT AUTHORITY\SYSTEM"] {
        assert!(!acl.contains(other), "{other} is in the key's ACL: {acl}");
    }
}
