// SPDX-License-Identifier: MIT
//! Rotation bookkeeping (plan C4): which rotations are running, who has drained, and the limits.
//!
//! A rotation replaces a service's file. `RESIZE` runs a drain barrier first: every member of the old
//! file flushes and acknowledges (or is dropped as a straggler at the deadline). `REVOCATION` has
//! no barrier. The protocol is `commy-wire`'s `spec/rotation.md`; this module only tracks state.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// Why a rotation could not start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The service is already rotating.
    InProgress,
    /// The service rotated too recently.
    TooSoon,
    /// The requester has started too many rotations lately.
    RequesterLimit,
}

/// Where a resize is: waiting for SDKs to drain, or waiting for the requester to fill the new file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Draining,
    Filling,
}

/// The requester's answer once the new file is prepared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillResult {
    Filled,
    Aborted,
}

/// A running rotation.
#[derive(Clone, Debug)]
pub struct Active {
    pub rotation_id: String,
    pub file_id: u32,
    /// The session that asked for the rotation, and the queue to reach it.
    pub requester_session: Option<String>,
    pub requester_tx: Option<crate::server::forwarding::OutTx>,
    pub phase: Phase,
    pub fill_result: Option<FillResult>,
    /// Sessions that must drain (resize only).
    pub expected: HashSet<String>,
    pub drained: HashSet<String>,
    pub notify: Arc<Notify>,
}

impl Active {
    pub fn all_drained(&self) -> bool {
        self.expected.is_subset(&self.drained)
    }
}

/// Default time a rotation of one service must be spaced from the previous one.
pub const DEFAULT_MIN_INTERVAL: Duration = Duration::from_secs(1);

/// Most rotations one requester may start per [`REQUESTER_WINDOW`].
pub const REQUESTER_CAP: usize = 6;
pub const REQUESTER_WINDOW: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Inner {
    active: HashMap<String, Active>,
    last_started: HashMap<String, Instant>,
    per_requester: HashMap<String, VecDeque<Instant>>,
}

#[derive(Default)]
pub struct Rotations {
    inner: Mutex<Inner>,
}

impl Rotations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a rotation of `service_id`, or say why not. On success returns the new rotation.
    pub fn begin(
        &self,
        service_id: &str,
        requester: &str,
        file_id: u32,
        expected: HashSet<String>,
        min_interval: Duration,
        now: Instant,
    ) -> Result<Active, Refusal> {
        self.begin_inner(service_id, requester, file_id, expected, Some(min_interval), now)
    }

    /// Start a rotation that access revocation requires. Not subject to the spacing or the
    /// per-requester limit (access must end now), but still one rotation per service at a time.
    pub fn begin_forced(
        &self,
        service_id: &str,
        requester: &str,
        file_id: u32,
        expected: HashSet<String>,
        now: Instant,
    ) -> Result<Active, Refusal> {
        self.begin_inner(service_id, requester, file_id, expected, None, now)
    }

    fn begin_inner(
        &self,
        service_id: &str,
        requester: &str,
        file_id: u32,
        expected: HashSet<String>,
        limits: Option<Duration>,
        now: Instant,
    ) -> Result<Active, Refusal> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.active.contains_key(service_id) {
            return Err(Refusal::InProgress);
        }
        if let Some(min_interval) = limits {
            if let Some(last) = g.last_started.get(service_id) {
                if now.saturating_duration_since(*last) < min_interval {
                    return Err(Refusal::TooSoon);
                }
            }
            let window = g.per_requester.entry(requester.to_string()).or_default();
            while window
                .front()
                .is_some_and(|t| now.saturating_duration_since(*t) >= REQUESTER_WINDOW)
            {
                window.pop_front();
            }
            if window.len() >= REQUESTER_CAP {
                return Err(Refusal::RequesterLimit);
            }
            window.push_back(now);
        }
        let active = Active {
            rotation_id: uuid::Uuid::new_v4().to_string(),
            file_id,
            requester_session: None,
            requester_tx: None,
            phase: Phase::Draining,
            fill_result: None,
            expected,
            drained: HashSet::new(),
            notify: Arc::new(Notify::new()),
        };
        g.last_started.insert(service_id.to_string(), now);
        g.active.insert(service_id.to_string(), active.clone());
        Ok(active)
    }

    /// Record that `session_id` drained. Fails if the rotation is not running, `file_id` is not the
    /// file being rotated, or the session was not asked to drain.
    pub fn mark_drained(
        &self,
        rotation_id: &str,
        file_id: u32,
        session_id: &str,
    ) -> Result<(), &'static str> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(active) = g.active.values_mut().find(|a| a.rotation_id == rotation_id) else {
            return Err("no such rotation is running");
        };
        if active.file_id != file_id {
            return Err("file_id is not the file being rotated");
        }
        if !active.expected.contains(session_id) {
            return Err("this session was not asked to drain");
        }
        active.drained.insert(session_id.to_string());
        active.notify.notify_one();
        Ok(())
    }

    /// Record who asked for the rotation and how to reach them.
    pub fn set_requester(
        &self,
        service_id: &str,
        session_id: &str,
        tx: Option<crate::server::forwarding::OutTx>,
    ) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(a) = g.active.get_mut(service_id) {
            a.requester_session = Some(session_id.to_string());
            a.requester_tx = tx;
        }
    }

    /// The new file is prepared: from now the requester may answer `Filled` or `Aborted`.
    pub fn set_filling(&self, service_id: &str) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(a) = g.active.get_mut(service_id) {
            a.phase = Phase::Filling;
        }
    }

    /// The requester's answer. Only the requester may give it, only once, and only after the new
    /// file was prepared.
    pub fn respond(
        &self,
        rotation_id: &str,
        session_id: &str,
        result: FillResult,
    ) -> Result<(), &'static str> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(a) = g.active.values_mut().find(|a| a.rotation_id == rotation_id) else {
            return Err("no such rotation is running");
        };
        if a.requester_session.as_deref() != Some(session_id) {
            return Err("only the requester may answer");
        }
        if a.phase != Phase::Filling {
            return Err("the new file is not prepared yet");
        }
        if a.fill_result.is_some() {
            return Err("this rotation was already answered");
        }
        a.fill_result = Some(result);
        a.notify.notify_one();
        Ok(())
    }

    /// A snapshot of the running rotation of a service.
    pub fn active(&self, service_id: &str) -> Option<Active> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.active.get(service_id).cloned()
    }

    /// End the rotation of a service, returning its final state.
    pub fn finish(&self, service_id: &str) -> Option<Active> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.active.remove(service_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn one_rotation_per_service_at_a_time() {
        let r = Rotations::new();
        let now = t0();
        r.begin("svc", "alice", 1, set(&[]), Duration::ZERO, now).unwrap();
        assert_eq!(
            r.begin("svc", "alice", 1, set(&[]), Duration::ZERO, now).unwrap_err(),
            Refusal::InProgress
        );
        assert!(r.begin("other", "alice", 2, set(&[]), Duration::ZERO, now).is_ok());
        r.finish("svc").unwrap();
        assert!(r.begin("svc", "alice", 1, set(&[]), Duration::ZERO, now).is_ok());
    }

    #[test]
    fn a_service_cannot_rotate_again_inside_the_interval() {
        let r = Rotations::new();
        let now = t0();
        let gap = Duration::from_secs(10);
        r.begin("svc", "a", 1, set(&[]), gap, now).unwrap();
        assert!(r.finish("svc").is_some());
        assert_eq!(
            r.begin("svc", "a", 1, set(&[]), gap, now + Duration::from_secs(9)).unwrap_err(),
            Refusal::TooSoon
        );
        assert!(r.begin("svc", "a", 1, set(&[]), gap, now + Duration::from_secs(10)).is_ok());
    }

    #[test]
    fn one_requester_cannot_start_unlimited_rotations() {
        let r = Rotations::new();
        let now = t0();
        for i in 0..REQUESTER_CAP {
            let svc = format!("svc{i}");
            r.begin(&svc, "mallory", 1, set(&[]), Duration::ZERO, now).unwrap();
        }
        assert_eq!(
            r.begin("svc-x", "mallory", 1, set(&[]), Duration::ZERO, now).unwrap_err(),
            Refusal::RequesterLimit
        );
        // another requester is unaffected, and the window slides
        assert!(r.begin("svc-y", "alice", 1, set(&[]), Duration::ZERO, now).is_ok());
        let later = now + REQUESTER_WINDOW;
        assert!(r.begin("svc-x", "mallory", 1, set(&[]), Duration::ZERO, later).is_ok());
    }

    #[test]
    fn a_forced_rotation_skips_the_spacing_and_the_requester_limit_but_not_the_one_at_a_time_rule() {
        let r = Rotations::new();
        let now = t0();
        for i in 0..REQUESTER_CAP + 2 {
            let svc = format!("svc{i}");
            r.begin_forced(&svc, "root", 1, set(&[]), now).unwrap();
            assert!(r.finish(&svc).is_some());
        }
        // spacing: the same service again at once
        r.begin_forced("svc0", "root", 1, set(&[]), now).unwrap();
        assert!(r.finish("svc0").is_some());
        r.begin_forced("svc0", "root", 1, set(&[]), now).unwrap();
        assert_eq!(
            r.begin_forced("svc0", "root", 1, set(&[]), now).unwrap_err(),
            Refusal::InProgress
        );
    }

    #[test]
    fn drain_tracking_requires_every_expected_session() {
        let r = Rotations::new();
        let a = r
            .begin("svc", "a", 7, set(&["s1", "s2"]), Duration::ZERO, t0())
            .unwrap();
        assert!(!r.active("svc").unwrap().all_drained());
        r.mark_drained(&a.rotation_id, 7, "s1").unwrap();
        assert!(!r.active("svc").unwrap().all_drained());
        r.mark_drained(&a.rotation_id, 7, "s2").unwrap();
        assert!(r.active("svc").unwrap().all_drained());
    }

    #[test]
    fn drain_acknowledgements_are_checked() {
        let r = Rotations::new();
        let a = r.begin("svc", "a", 7, set(&["s1"]), Duration::ZERO, t0()).unwrap();
        assert!(r.mark_drained("no-such-rotation", 7, "s1").is_err());
        assert!(r.mark_drained(&a.rotation_id, 8, "s1").is_err(), "wrong file");
        assert!(r.mark_drained(&a.rotation_id, 7, "stranger").is_err(), "not asked");
        assert!(!r.active("svc").unwrap().all_drained());
    }

    #[test]
    fn only_the_requester_may_answer_and_only_after_prepare_and_once() {
        let r = Rotations::new();
        let a = r.begin("svc", "a", 7, set(&[]), Duration::ZERO, t0()).unwrap();
        r.set_requester("svc", "req", None);
        assert!(r.respond(&a.rotation_id, "req", FillResult::Filled).is_err(), "not prepared yet");
        r.set_filling("svc");
        assert!(r.respond(&a.rotation_id, "stranger", FillResult::Filled).is_err());
        assert!(r.respond("nope", "req", FillResult::Filled).is_err());
        r.respond(&a.rotation_id, "req", FillResult::Filled).unwrap();
        assert!(r.respond(&a.rotation_id, "req", FillResult::Aborted).is_err(), "only once");
        assert_eq!(r.active("svc").unwrap().fill_result, Some(FillResult::Filled));
    }

    #[test]
    fn an_empty_barrier_is_already_drained() {
        let r = Rotations::new();
        r.begin("svc", "a", 1, set(&[]), Duration::ZERO, t0()).unwrap();
        assert!(r.active("svc").unwrap().all_drained());
    }
}
