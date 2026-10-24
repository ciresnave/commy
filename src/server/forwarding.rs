// SPDX-License-Identifier: MIT
//! Range forwarding (plan C3): who is on which file, and what to send them.
//!
//! A session joins a file as a **local** mapper (it proved it shares the filesystem, see
//! `local_mapping`) or a **remote** mirror (it resynced over WSS). A flush from one member is
//! forwarded to every other member of the file: local members get the changed ranges only, remote
//! members get the ranges plus the bytes read from the file at forwarding time.

use commy_wire::ByteRange;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;

/// A message queued for one connection. The connection loop sends it as text or binary.
#[derive(Debug)]
pub enum Outbound {
    Text(serde_json::Value),
    Binary(Vec<u8>),
}

/// The sending half of a connection's outbound queue.
pub type OutTx = UnboundedSender<Outbound>;

/// How a member reaches the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Maps the same file; sees every store at once.
    Local,
    /// Holds a mirror; needs the bytes.
    Remote,
}

/// A session on a file.
#[derive(Clone, Debug)]
pub struct MemberInfo {
    pub session_id: String,
    pub client_id: Option<String>,
    pub mode: Mode,
    pub tx: OutTx,
}

#[derive(Default)]
struct Inner {
    /// `file_id` to (`session_id` to member).
    files: HashMap<u32, HashMap<String, MemberInfo>>,
}

/// Membership of sessions in files. Cheap to call from handlers: one short lock, no awaits.
#[derive(Default)]
pub struct Registry {
    inner: Mutex<Inner>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add (or change) a session's membership of a file.
    pub fn join(
        &self,
        file_id: u32,
        session_id: &str,
        client_id: Option<&str>,
        mode: Mode,
        tx: OutTx,
    ) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.files.entry(file_id).or_default().insert(
            session_id.to_string(),
            MemberInfo {
                session_id: session_id.to_string(),
                client_id: client_id.map(str::to_string),
                mode,
                tx,
            },
        );
    }

    /// The live members of a file (closed queues pruned).
    pub fn members(&self, file_id: u32) -> Vec<MemberInfo> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(members) = g.files.get_mut(&file_id) else {
            return Vec::new();
        };
        members.retain(|_, m| !m.tx.is_closed());
        members.values().cloned().collect()
    }

    /// Remove and return every member of a file (the file was rotated away).
    pub fn take_file(&self, file_id: u32) -> Vec<MemberInfo> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.files
            .remove(&file_id)
            .map(|m| m.into_values().collect())
            .unwrap_or_default()
    }

    /// Remove a session from every file (on disconnect).
    pub fn leave_session(&self, session_id: &str) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.files.retain(|_, members| {
            members.remove(session_id);
            !members.is_empty()
        });
    }

    /// Every member of the file except `except`, with closed queues pruned first.
    pub fn receivers(&self, file_id: u32, except: &str) -> Vec<(Mode, OutTx)> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(members) = g.files.get_mut(&file_id) else {
            return Vec::new();
        };
        members.retain(|_, m| !m.tx.is_closed());
        let out = members
            .iter()
            .filter(|(id, _)| id.as_str() != except)
            .map(|(_, m)| (m.mode, m.tx.clone()))
            .collect();
        if members.is_empty() {
            g.files.remove(&file_id);
        }
        out
    }

    /// How many sessions are on the file (for tests and diagnostics).
    pub fn member_count(&self, file_id: u32) -> usize {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.files.get(&file_id).map_or(0, |m| m.len())
    }
}

/// Sort ranges and merge any that overlap or touch, so a flush is a minimal sorted list.
/// Empty ranges are dropped.
pub fn merge_ranges(mut ranges: Vec<ByteRange>) -> Vec<ByteRange> {
    ranges.retain(|r| r.len > 0);
    ranges.sort_by_key(|r| r.offset);
    let mut out: Vec<ByteRange> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match out.last_mut() {
            Some(last) if r.offset <= last.offset.saturating_add(last.len) => {
                let end = r.offset.saturating_add(r.len);
                let last_end = last.offset.saturating_add(last.len);
                last.len = end.max(last_end) - last.offset;
            }
            _ => out.push(r),
        }
    }
    out
}

/// Read `ranges` of the file at `path`, returning one binary range frame per run of at most
/// `MAX_PAYLOAD` bytes, all stamped with `file_id` and `seq`.
pub fn read_frames(
    path: &str,
    file_id: u32,
    seq: u64,
    ranges: &[ByteRange],
) -> std::io::Result<Vec<Vec<u8>>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let mut frames = Vec::new();
    for r in ranges {
        let mut buf = vec![0u8; r.len as usize];
        file.seek(SeekFrom::Start(r.offset))?;
        file.read_exact(&mut buf)?;
        for f in commy_wire::RangeFrame::split(file_id, seq, r.offset, &buf) {
            frames.push(f.encode());
        }
    }
    Ok(frames)
}
