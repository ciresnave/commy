//! Durable state (plan R5): the service registry and the grants, on PersistAnt.
//!
//! Without this, a restart forgets every service id (the files are orphaned) and every grant. What
//! is stored:
//!
//! - one record per service, `services/<service_id>` (tenant, name, file path, `file_id`,
//!   generation, whether it is open to local mapping), replaced atomically on every change;
//! - the grants as an append-only log of operations, `grants/<seq>`, replayed in order at start-up.
//!
//! Not stored, on purpose: sessions, nonces, rotations in flight, per-file sequence numbers (clients
//! resynchronise after a restart), and the permissions a session carries from authentication (those
//! are re-imported when the client authenticates again).
//!
//! Writes go through before the change takes effect and a failed write refuses the change (fail
//! closed): a revoke that cannot be recorded must not be applied, or a restart would give the access back.

use persistant::blocking::Store;
use persistant::{Config, Need, Needs, Record};
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A service as stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceRow {
    pub tenant: String,
    pub name: String,
    pub path: String,
    pub file_id: u32,
    pub generation: u64,
    pub trusted_local: bool,
}

impl Record for ServiceRow {
    const KIND: &'static str = "commy-service";
    const SCHEMA: u32 = 1;
}

/// Grant or take away.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpKind {
    Grant,
    Revoke,
}

/// One change to the grants, as stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantOp {
    pub kind: OpKind,
    pub tenant: String,
    pub client: String,
    /// `None` = tenant-wide.
    pub service: Option<String>,
    /// Wire names (`service:read`, `tenant:admin`, ...); empty on a revoke means all at that scope.
    pub permissions: Vec<String>,
}

/// Marks a service as deleted. A write (atomic replace), so recording a deletion is as durable and as
/// testable as recording anything else; the service row is removed afterwards, best effort.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    pub tenant: String,
    pub name: String,
}

impl Record for Tombstone {
    const KIND: &'static str = "commy-service-tombstone";
    const SCHEMA: u32 = 1;
}

impl Record for GrantOp {
    const KIND: &'static str = "commy-grant-op";
    const SCHEMA: u32 = 1;
}

/// Why a read or write of the state failed.
#[derive(Debug)]
pub struct PersistError(pub String);

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "state store: {}", self.0)
    }
}

impl std::error::Error for PersistError {}

fn err<E: std::fmt::Display>(e: E) -> PersistError {
    PersistError(e.to_string())
}

/// The state store.
pub struct Persistence {
    store: Store,
    next_seq: Mutex<u64>,
}

const PAGE: usize = 256;

fn needs() -> Needs {
    Needs::new()
        .with(Need::Read)
        .with(Need::Write)
        .with(Need::Delete)
        .with(Need::Stat)
        .with(Need::List)
        .with(Need::AtomicReplace)
}

impl Persistence {
    /// Open (creating it if need be) the state under `dir`. The directory is private to the server
    /// and administrators: it holds the access decisions.
    pub fn open(dir: &Path) -> Result<Self, PersistError> {
        crate::private_dir::create_secret(dir).map_err(err)?;
        let root: PathBuf = dir.join("data");
        let scratch: PathBuf = dir.join("tmp");
        std::fs::create_dir_all(&root).map_err(err)?;
        std::fs::create_dir_all(&scratch).map_err(err)?;
        Self::open_with(Config::Fs {
            root,
            atomic_write_dir: Some(scratch),
        })
    }

    /// Open on any PersistAnt backend (tests use memory and fault-injecting fakes).
    pub fn open_with(config: Config) -> Result<Self, PersistError> {
        let store = Store::open(config, needs()).map_err(err)?;
        let this = Persistence {
            store,
            next_seq: Mutex::new(0),
        };
        let next = this.keys("grants/")?.last().map_or(0, |k| seq_of(k) + 1);
        *this.next_seq.lock().unwrap_or_else(|e| e.into_inner()) = next;
        Ok(this)
    }

    fn keys(&self, prefix: &str) -> Result<Vec<String>, PersistError> {
        let mut out = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let page = self
                .store
                .list(prefix, after.as_deref(), NonZeroUsize::new(PAGE).unwrap())
                .map_err(err)?;
            out.extend(page.entries.iter().map(|e| e.key.clone()));
            match page.next {
                Some(c) => after = Some(c),
                None => break,
            }
        }
        out.sort();
        Ok(out)
    }

    /// Record (or replace) a service.
    pub fn put_service(&self, id: &str, row: &ServiceRow) -> Result<(), PersistError> {
        self.store
            .put_record(&format!("services/{id}"), row)
            .map_err(err)
    }

    /// Forget a service. The tombstone is written first (so a failure leaves the service in place);
    /// the row is then removed, and a failure to remove it is harmless because the tombstone wins.
    pub fn delete_service(&self, id: &str, tenant: &str, name: &str) -> Result<(), PersistError> {
        self.store
            .put_record(
                &format!("deleted/{id}"),
                &Tombstone {
                    tenant: tenant.to_string(),
                    name: name.to_string(),
                },
            )
            .map_err(err)?;
        let _ = self.store.delete(&format!("services/{id}"));
        Ok(())
    }

    /// Every stored service, with its id, except the deleted ones.
    pub fn load_services(&self) -> Result<Vec<(String, ServiceRow)>, PersistError> {
        let mut out = Vec::new();
        let deleted: std::collections::HashSet<String> = self
            .keys("deleted/")?
            .into_iter()
            .map(|k| k.trim_start_matches("deleted/").to_string())
            .collect();
        for key in self.keys("services/")? {
            let id = key.trim_start_matches("services/").to_string();
            if deleted.contains(&id) {
                continue;
            }
            if let Some(row) = self.store.get_record::<ServiceRow>(&key).map_err(err)? {
                out.push((id, row));
            }
        }
        Ok(out)
    }

    /// Append a grant operation; returns once it is durable.
    pub fn append_grant_op(&self, op: &GrantOp) -> Result<u64, PersistError> {
        let mut seq = self.next_seq.lock().unwrap_or_else(|e| e.into_inner());
        self.store
            .put_record(&format!("grants/{:020}", *seq), op)
            .map_err(err)?;
        let used = *seq;
        *seq += 1;
        Ok(used)
    }

    /// Every stored grant operation, oldest first.
    pub fn load_grant_ops(&self) -> Result<Vec<GrantOp>, PersistError> {
        let mut out = Vec::new();
        for key in self.keys("grants/")? {
            if let Some(op) = self.store.get_record::<GrantOp>(&key).map_err(err)? {
                out.push(op);
            }
        }
        Ok(out)
    }
}

fn seq_of(key: &str) -> u64 {
    key.trim_start_matches("grants/").parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(n: u32) -> ServiceRow {
        ServiceRow {
            tenant: "t".into(),
            name: format!("svc{n}"),
            path: format!("tenant_t/service_{n}.mem"),
            file_id: n,
            generation: 0,
            trusted_local: false,
        }
    }

    fn op(client: &str) -> GrantOp {
        GrantOp {
            kind: OpKind::Grant,
            tenant: "t".into(),
            client: client.into(),
            service: Some("svc".into()),
            permissions: vec!["service:read".into()],
        }
    }

    #[test]
    fn services_round_trip_and_replace_and_delete() {
        let p = Persistence::open_with(Config::Memory).unwrap();
        p.put_service("a", &row(1)).unwrap();
        p.put_service("b", &row(2)).unwrap();
        let mut changed = row(1);
        changed.generation = 3;
        changed.trusted_local = true;
        p.put_service("a", &changed).unwrap();
        p.delete_service("b", "t", "svc2").unwrap();
        let all = p.load_services().unwrap();
        assert_eq!(all, vec![("a".to_string(), changed)]);
    }

    #[test]
    fn a_deleted_service_stays_deleted_even_if_its_row_comes_back() {
        let p = Persistence::open_with(Config::Memory).unwrap();
        p.put_service("a", &row(1)).unwrap();
        p.delete_service("a", "t", "svc1").unwrap();
        assert!(p.load_services().unwrap().is_empty());
        // a row written again under the same id (a stale copy) is still ignored: the tombstone wins
        p.put_service("a", &row(1)).unwrap();
        assert!(p.load_services().unwrap().is_empty());
    }

    #[test]
    fn a_deletion_that_cannot_be_recorded_leaves_the_service_stored() {
        let p = Persistence::open_with(Config::Faulty {
            inner: Box::new(Config::Memory),
            faults: persistant::Faults::fail_writes_after(1),
        })
        .unwrap();
        p.put_service("a", &row(1)).unwrap();
        assert!(p.delete_service("a", "t", "svc1").is_err());
        assert_eq!(p.load_services().unwrap().len(), 1);
    }

    #[test]
    fn grant_ops_keep_their_order() {
        let p = Persistence::open_with(Config::Memory).unwrap();
        for c in ["a", "b", "c"] {
            p.append_grant_op(&op(c)).unwrap();
        }
        let clients: Vec<_> = p.load_grant_ops().unwrap().into_iter().map(|o| o.client).collect();
        assert_eq!(clients, ["a", "b", "c"]);
    }

    #[test]
    fn more_than_one_page_of_ops_loads_in_order() {
        let p = Persistence::open_with(Config::Memory).unwrap();
        let n = PAGE + 40;
        for i in 0..n {
            p.append_grant_op(&op(&format!("c{i:04}"))).unwrap();
        }
        let ops = p.load_grant_ops().unwrap();
        assert_eq!(ops.len(), n);
        assert_eq!(ops[0].client, "c0000");
        assert_eq!(ops[n - 1].client, format!("c{:04}", n - 1));
    }

    #[test]
    fn a_reopened_store_continues_the_sequence_and_keeps_everything() {
        let dir = tempfile::tempdir().unwrap();
        {
            let p = Persistence::open(dir.path()).unwrap();
            p.append_grant_op(&op("first")).unwrap();
            p.put_service("a", &row(1)).unwrap();
        }
        let p = Persistence::open(dir.path()).unwrap();
        assert_eq!(p.append_grant_op(&op("second")).unwrap(), 1, "the sequence continues");
        let clients: Vec<_> = p.load_grant_ops().unwrap().into_iter().map(|o| o.client).collect();
        assert_eq!(clients, ["first", "second"]);
        assert_eq!(p.load_services().unwrap().len(), 1);
    }

    #[test]
    fn a_failed_write_is_reported_and_does_not_advance_the_sequence() {
        // The first write goes through, the second and later ones fail.
        let p = Persistence::open_with(Config::Faulty {
            inner: Box::new(Config::Memory),
            faults: persistant::Faults::fail_writes_after(1),
        })
        .unwrap();
        assert_eq!(p.append_grant_op(&op("kept")).unwrap(), 0);
        assert!(p.append_grant_op(&op("lost")).is_err());
        assert!(p.append_grant_op(&op("lost-too")).is_err());
        let clients: Vec<_> = p.load_grant_ops().unwrap().into_iter().map(|o| o.client).collect();
        assert_eq!(clients, ["kept"], "a failed append leaves nothing behind");
    }
}
