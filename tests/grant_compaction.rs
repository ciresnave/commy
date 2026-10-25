// SPDX-License-Identifier: MIT
//! Grant-log compaction (plan R23): a server that compacts and restarts answers every access
//! question as one that replayed the whole log, and the log stops growing.

use commy::server::grants::{Action, Spec};
use commy::Server;
use std::path::Path;

const TENANT: &str = "t";
const CLIENTS: [&str; 3] = ["ann", "bob", "cy"];
const SERVICES: [&str; 3] = ["s1", "s2", "s3"];

/// xorshift: the same sequence on every run, so a failure reproduces.
struct Rng(u64);

impl Rng {
    fn next(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

fn some_specs(rng: &mut Rng, tenant_wide: bool) -> Vec<Spec> {
    let mut pool: Vec<Spec> = Action::ALL.iter().map(|a| Spec::Service(*a)).collect();
    if tenant_wide {
        pool.push(Spec::TenantAdmin);
    }
    let n = rng.next(3); // 0 = everything on a revoke
    (0..n).map(|_| pool[rng.next(pool.len())]).collect()
}

fn do_random_ops(server: &Server, rng: &mut Rng, n: usize) {
    for _ in 0..n {
        let client = CLIENTS[rng.next(CLIENTS.len())];
        let service = (rng.next(3) != 0).then(|| SERVICES[rng.next(SERVICES.len())]);
        let specs = some_specs(rng, service.is_none());
        if rng.next(2) == 0 {
            if !specs.is_empty() {
                server.grant(TENANT, client, service, &specs).unwrap();
            }
        } else {
            server.revoke(TENANT, client, service, &specs).unwrap();
        }
    }
}

/// Every answer the grant store can give about the three clients and services.
fn answers(server: &Server) -> Vec<String> {
    let mut out = Vec::new();
    for c in CLIENTS {
        out.push(format!("{c} admin={}", server.grants.is_tenant_admin(TENANT, c)));
        for s in SERVICES {
            for a in Action::ALL {
                out.push(format!("{c} {s} {a:?}={}", server.grants.allows(TENANT, c, s, a)));
            }
        }
    }
    out.push(format!("{:?}", server.grants.list(TENANT, None, None)));
    out
}

fn open(dir: &Path, compact_after: u64) -> Server {
    let mut s = Server::new();
    s.grant_compact_after = compact_after;
    s.enable_persistence(dir).unwrap();
    s
}

fn op_files(dir: &Path) -> usize {
    std::fs::read_dir(dir.join("data").join("grants")).map_or(0, |d| d.count())
}

#[test]
fn a_compacted_log_answers_as_the_whole_log_does() {
    for seed in [1u64, 7, 99, 12345, 777] {
        let dir = tempfile::tempdir().unwrap();
        let before = {
            let s = open(dir.path(), 0);
            do_random_ops(&s, &mut Rng(seed * 2654435761), 120);
            answers(&s)
        };
        assert!(op_files(dir.path()) > 50, "seed {seed}: the log is long");
        assert_eq!(answers(&open(dir.path(), 0)), before, "seed {seed}: full replay");

        let folded = open(dir.path(), 0).compact_grants().unwrap();
        assert!(folded > 50, "seed {seed}: {folded} operations folded");
        assert_eq!(op_files(dir.path()), 0, "seed {seed}: nothing left to replay");
        assert_eq!(answers(&open(dir.path(), 0)), before, "seed {seed}: replay from the snapshot");

        // and the snapshot keeps working: more changes, a second compaction, a restart
        let after = {
            let s = open(dir.path(), 0);
            do_random_ops(&s, &mut Rng(seed + 5), 40);
            let a = answers(&s);
            assert_ne!(a, before, "seed {seed}: the new operations changed something");
            a
        };
        assert_eq!(answers(&open(dir.path(), 0)), after, "seed {seed}: snapshot plus later operations");
        open(dir.path(), 0).compact_grants().unwrap();
        assert_eq!(answers(&open(dir.path(), 0)), after, "seed {seed}: compacted twice");
    }
}

#[test]
fn the_log_compacts_itself_at_the_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let after = {
        let s = open(dir.path(), 10);
        do_random_ops(&s, &mut Rng(42), 95);
        answers(&s)
    };
    assert!(op_files(dir.path()) < 10, "{} operation files left", op_files(dir.path()));
    assert_eq!(answers(&open(dir.path(), 10)), after);
}

#[test]
fn a_session_import_does_not_end_up_in_a_snapshot() {
    use commy::auth::{Permission, PermissionSet};
    let dir = tempfile::tempdir().unwrap();
    let s = open(dir.path(), 0);
    s.grant(TENANT, "ann", Some("s1"), &[Spec::Service(Action::Read)]).unwrap();
    let mut perms = PermissionSet::new();
    perms.grant(Permission::ServiceAdmin);
    s.grants.import_session(TENANT, "bob", &perms).unwrap();
    assert!(s.grants.allows(TENANT, "bob", "s1", Action::Admin), "the live session has it");
    s.compact_grants().unwrap();
    let restarted = open(dir.path(), 0);
    assert!(!restarted.grants.allows(TENANT, "bob", "s1", Action::Admin), "authentication imports it again");
    assert!(restarted.grants.allows(TENANT, "ann", "s1", Action::Read));
}
