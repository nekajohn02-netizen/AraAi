//! Event Watcher: "when X happens on this device, run this mission."
//!
//! Rust never listens to anything itself — it can't; Android broadcasts and
//! `NetworkCallback`s only exist on the Kotlin side. This crate owns the
//! *rules* ("on battery-low, do this") and, critically, the cooldown that
//! keeps a flapping signal (Wi-Fi dropping in and out a dozen times a
//! minute) from spawning a storm of missions — exactly the kind of
//! resource/battery abuse this project has been auditing against all
//! session. The host calls [`EventStore::due_for`] whenever it observes an
//! event; everything about *running* the resulting mission is, as with
//! [`ara_scheduler`], the same tested `ara_missions` machinery as one the
//! user started by hand.

use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Same floor as `ara_scheduler::MIN_INTERVAL_MS`, same reasoning: a rule
/// can fire at most once per this many ms, no matter how often the
/// underlying signal flaps.
pub const MIN_COOLDOWN_MS: u64 = 15 * 60 * 1000;
pub const MAX_GOAL_CHARS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    BatteryLow,
    PowerConnected,
    PowerDisconnected,
    NetworkAvailable,
    NetworkLost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRule {
    pub id: String,
    pub kind: EventKind,
    pub goal: String,
    pub persona: String,
    pub max_steps: u32,
    pub max_duration_ms: u64,
    pub created_ms: u64,
    pub enabled: bool,
    pub last_fired_ms: Option<u64>,
    pub last_mission_id: Option<String>,
}

/// What firing one due rule means: go create exactly this mission. Same
/// shape as `ara_scheduler::FireRequest` and, like it, never mutates
/// anything on its own — the caller marks a rule fired only once the
/// mission is actually created, so a crash mid-dispatch just sees the same
/// due rule again on the next event, never silently drops it.
#[derive(Debug, Clone, PartialEq)]
pub struct FireRequest {
    pub rule_id: String,
    pub goal: String,
    pub persona: String,
    pub max_steps: u32,
    pub max_duration_ms: u64,
}

pub struct EventStore {
    dir: PathBuf,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

impl EventStore {
    pub fn open(dir: impl Into<PathBuf>) -> AraResult<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn save(&self, r: &EventRule) -> AraResult<()> {
        let tmp = self.dir.join(format!("{}.tmp", r.id));
        fs::write(&tmp, serde_json::to_vec(r).map_err(|e| AraError::Internal(e.to_string()))?)?;
        fs::rename(&tmp, self.path(&r.id))?;
        Ok(())
    }

    pub fn create(
        &self,
        kind: EventKind,
        goal: &str,
        persona: &str,
        max_steps: u32,
        max_duration_ms: u64,
        now_ms: u64,
    ) -> AraResult<EventRule> {
        let goal: String = goal
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(MAX_GOAL_CHARS)
            .collect();
        if goal.trim().is_empty() {
            return Err(AraError::InvalidInput("empty goal".into()));
        }
        let id = format!("{now_ms:x}-{}", self.list()?.len());
        let r = EventRule {
            id: id.clone(),
            kind,
            goal,
            persona: persona.to_string(),
            max_steps,
            max_duration_ms,
            created_ms: now_ms,
            enabled: true,
            last_fired_ms: None,
            last_mission_id: None,
        };
        self.save(&r)?;
        Ok(r)
    }

    pub fn get(&self, id: &str) -> AraResult<EventRule> {
        if !valid_id(id) {
            return Err(AraError::InvalidInput("bad rule id".into()));
        }
        let b = fs::read(self.path(id)).map_err(|_| AraError::NotFound("event rule".into()))?;
        serde_json::from_slice(&b).map_err(|_| AraError::InvalidInput("event rule file is corrupt".into()))
    }

    pub fn list(&self) -> AraResult<Vec<EventRule>> {
        let mut v: Vec<EventRule> = fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
            .filter_map(|e| fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice(&b).ok())
            .collect();
        v.sort_by(|a, b| a.created_ms.cmp(&b.created_ms).then(a.id.cmp(&b.id)));
        Ok(v)
    }

    pub fn delete(&self, id: &str) -> AraResult<()> {
        if !valid_id(id) {
            return Err(AraError::InvalidInput("bad rule id".into()));
        }
        match fs::remove_file(self.path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> AraResult<EventRule> {
        let mut r = self.get(id)?;
        r.enabled = enabled;
        self.save(&r)?;
        Ok(r)
    }

    /// Rules that should fire for an event of this `kind` that just
    /// happened: enabled, matching, and past their cooldown. Doesn't fire
    /// them — call `fire` for each one the caller actually creates a
    /// mission for.
    pub fn due_for(&self, kind: EventKind, now_ms: u64) -> AraResult<Vec<EventRule>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|r| {
                r.enabled
                    && r.kind == kind
                    && r.last_fired_ms.is_none_or(|t| now_ms.saturating_sub(t) >= MIN_COOLDOWN_MS)
            })
            .collect())
    }

    pub fn due_requests(&self, kind: EventKind, now_ms: u64) -> AraResult<Vec<FireRequest>> {
        Ok(self
            .due_for(kind, now_ms)?
            .into_iter()
            .map(|r| FireRequest {
                rule_id: r.id,
                goal: r.goal,
                persona: r.persona,
                max_steps: r.max_steps,
                max_duration_ms: r.max_duration_ms,
            })
            .collect())
    }

    /// Call after successfully creating `mission_id` for this rule. Unlike
    /// a one-shot scheduled task, an event rule stays enabled — "whenever
    /// this happens, do that" is inherently recurring.
    pub fn fire(&self, id: &str, now_ms: u64, mission_id: &str) -> AraResult<EventRule> {
        let mut r = self.get(id)?;
        r.last_fired_ms = Some(now_ms);
        r.last_mission_id = Some(mission_id.to_string());
        self.save(&r)?;
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, EventStore) {
        let d = tempfile::tempdir().unwrap();
        let s = EventStore::open(d.path()).unwrap();
        (d, s)
    }
    const H: u64 = 3_600_000;

    #[test]
    fn rejects_an_empty_goal() {
        let (_d, s) = store();
        assert!(s.create(EventKind::BatteryLow, "  ", "general", 10, H, 0).is_err());
    }

    #[test]
    fn a_fresh_rule_is_immediately_due_for_its_own_kind_only() {
        let (_d, s) = store();
        s.create(EventKind::BatteryLow, "dim the lights", "general", 10, H, 0).unwrap();
        assert_eq!(s.due_for(EventKind::BatteryLow, 0).unwrap().len(), 1);
        assert_eq!(s.due_for(EventKind::NetworkAvailable, 0).unwrap().len(), 0, "wrong kind never matches");
    }

    #[test]
    fn a_flapping_signal_cannot_spawn_a_mission_storm() {
        let (_d, s) = store();
        let r = s
            .create(EventKind::NetworkAvailable, "sync files", "general", 10, H, 0)
            .unwrap();
        s.fire(&r.id, 1_000, "mission-1").unwrap();
        // The network flaps on and off every few seconds right after.
        for t in [2_000, 5_000, 30_000, MIN_COOLDOWN_MS / 2] {
            assert!(
                s.due_for(EventKind::NetworkAvailable, t).unwrap().is_empty(),
                "still inside the cooldown at t={t}"
            );
        }
        assert_eq!(s.due_for(EventKind::NetworkAvailable, 1_000 + MIN_COOLDOWN_MS).unwrap().len(), 1);
    }

    #[test]
    fn due_requests_never_mutates_and_carries_mission_parameters() {
        let (_d, s) = store();
        s.create(EventKind::PowerConnected, "ابدأ النسخ الاحتياطي", "general", 20, H, 0).unwrap();
        let r1 = s.due_requests(EventKind::PowerConnected, 0).unwrap();
        let r2 = s.due_requests(EventKind::PowerConnected, 0).unwrap();
        assert_eq!(r1, r2, "asking twice without firing must not change anything");
        assert_eq!(r1[0].goal, "ابدأ النسخ الاحتياطي");
        assert_eq!(r1[0].max_steps, 20);
    }

    #[test]
    fn disabling_a_rule_removes_it_from_due_and_re_enabling_restores_it() {
        let (_d, s) = store();
        let r = s.create(EventKind::PowerDisconnected, "g", "general", 10, H, 0).unwrap();
        s.set_enabled(&r.id, false).unwrap();
        assert!(s.due_for(EventKind::PowerDisconnected, 0).unwrap().is_empty());
        s.set_enabled(&r.id, true).unwrap();
        assert_eq!(s.due_for(EventKind::PowerDisconnected, 0).unwrap().len(), 1);
    }

    #[test]
    fn an_event_rule_stays_enabled_after_firing_unlike_a_one_shot_scheduled_task() {
        let (_d, s) = store();
        let r = s.create(EventKind::BatteryLow, "g", "general", 10, H, 0).unwrap();
        let fired = s.fire(&r.id, 0, "mission-1").unwrap();
        assert!(fired.enabled, "event rules are recurring by nature");
    }

    #[test]
    fn rejects_path_traversal_ids_and_delete_is_idempotent() {
        let (_d, s) = store();
        assert!(s.get("../../etc/passwd").is_err());
        let r = s.create(EventKind::BatteryLow, "g", "general", 10, H, 0).unwrap();
        s.delete(&r.id).unwrap();
        s.delete(&r.id).unwrap(); // no-op, not an error
        assert!(s.list().unwrap().is_empty());
    }
}
