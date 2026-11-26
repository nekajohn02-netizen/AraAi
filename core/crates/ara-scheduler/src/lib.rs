//! Schedules a mission to start later — once, or repeating on an interval.
//!
//! This crate only ever answers one question: "is it time yet, and if so,
//! what mission should be created?" It never runs anything itself and knows
//! nothing about models, tools, or steps — `ara_missions` still owns all of
//! that, plus every safety property a running mission already has (budget,
//! retries, crash-safe checkpointing). Firing a task is nothing more than
//! "hand back the parameters for `MissionStore::create`", so a scheduled
//! mission is exactly as safe as one the user started by hand.

use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// However useful a reminder feels, firing more often than this is a
/// battery/heat concern, not a product feature — matches Android
/// `PeriodicWorkRequest`'s own 15-minute floor, so the two layers agree.
pub const MIN_INTERVAL_MS: u64 = 15 * 60 * 1000;
pub const MAX_GOAL_CHARS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "every_ms")]
pub enum Recurrence {
    Once,
    Repeating(u64),
}

impl Recurrence {
    fn interval(self) -> AraResult<Option<u64>> {
        match self {
            Recurrence::Once => Ok(None),
            Recurrence::Repeating(ms) if ms >= MIN_INTERVAL_MS => Ok(Some(ms)),
            Recurrence::Repeating(_) => {
                Err(AraError::InvalidInput(format!("repeat interval must be >= {MIN_INTERVAL_MS}ms")))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub goal: String,
    pub persona: String,
    pub max_steps: u32,
    pub max_duration_ms: u64,
    pub created_ms: u64,
    pub next_fire_ms: u64,
    pub recurrence: Recurrence,
    pub enabled: bool,
    pub last_fired_ms: Option<u64>,
    /// Mission id created by the most recent firing, if any.
    pub last_mission_id: Option<String>,
}

/// What firing one due task means to the caller: go create exactly this
/// mission. Deliberately the same shape `ara_missions::MissionStore::create`
/// takes — the scheduler hands back parameters, never a mission itself.
#[derive(Debug, Clone, PartialEq)]
pub struct FireRequest {
    pub task_id: String,
    pub goal: String,
    pub persona: String,
    pub max_steps: u32,
    pub max_duration_ms: u64,
}

pub struct SchedulerStore {
    dir: PathBuf,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

impl SchedulerStore {
    pub fn open(dir: impl Into<PathBuf>) -> AraResult<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn save(&self, t: &ScheduledTask) -> AraResult<()> {
        let tmp = self.dir.join(format!("{}.tmp", t.id));
        fs::write(&tmp, serde_json::to_vec(t).map_err(|e| AraError::Internal(e.to_string()))?)?;
        fs::rename(&tmp, self.path(&t.id))?;
        Ok(())
    }

    pub fn create(
        &self,
        goal: &str,
        persona: &str,
        max_steps: u32,
        max_duration_ms: u64,
        fire_at_ms: u64,
        recurrence: Recurrence,
        now_ms: u64,
    ) -> AraResult<ScheduledTask> {
        let goal: String = goal
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(MAX_GOAL_CHARS)
            .collect();
        if goal.trim().is_empty() {
            return Err(AraError::InvalidInput("empty goal".into()));
        }
        if fire_at_ms <= now_ms {
            return Err(AraError::InvalidInput("scheduled time must be in the future".into()));
        }
        recurrence.interval()?; // validates the interval floor up front
        let id = format!("{now_ms:x}-{}", self.list()?.len());
        let t = ScheduledTask {
            id: id.clone(),
            goal,
            persona: persona.to_string(),
            max_steps,
            max_duration_ms,
            created_ms: now_ms,
            next_fire_ms: fire_at_ms,
            recurrence,
            enabled: true,
            last_fired_ms: None,
            last_mission_id: None,
        };
        self.save(&t)?;
        Ok(t)
    }

    pub fn get(&self, id: &str) -> AraResult<ScheduledTask> {
        if !valid_id(id) {
            return Err(AraError::InvalidInput("bad task id".into()));
        }
        let b = fs::read(self.path(id)).map_err(|_| AraError::NotFound("scheduled task".into()))?;
        serde_json::from_slice(&b).map_err(|_| AraError::InvalidInput("scheduled task file is corrupt".into()))
    }

    pub fn list(&self) -> AraResult<Vec<ScheduledTask>> {
        let mut v: Vec<ScheduledTask> = fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
            .filter_map(|e| fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice(&b).ok())
            .collect();
        v.sort_by(|a, b| a.next_fire_ms.cmp(&b.next_fire_ms).then(a.id.cmp(&b.id)));
        Ok(v)
    }

    pub fn delete(&self, id: &str) -> AraResult<()> {
        if !valid_id(id) {
            return Err(AraError::InvalidInput("bad task id".into()));
        }
        match fs::remove_file(self.path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> AraResult<ScheduledTask> {
        let mut t = self.get(id)?;
        t.enabled = enabled;
        self.save(&t)?;
        Ok(t)
    }

    /// Tasks due to fire right now, oldest-due first. Doesn't fire them —
    /// call `fire` for each one the caller actually creates a mission for,
    /// so a mission-creation failure never silently loses the task.
    pub fn due(&self, now_ms: u64) -> AraResult<Vec<ScheduledTask>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|t| t.enabled && t.next_fire_ms <= now_ms)
            .collect())
    }

    /// Call after successfully creating `mission_id` for this task. For a
    /// one-shot task, disables it (kept around so the user can see it fired
    /// and what it produced, like a finished mission). For a repeating one,
    /// advances to the next interval from *now*, not from the missed slot —
    /// if the device was off for days, this fires once on return and skips
    /// the backlog instead of flooding missions for every interval missed.
    pub fn fire(&self, id: &str, now_ms: u64, mission_id: &str) -> AraResult<ScheduledTask> {
        let mut t = self.get(id)?;
        t.last_fired_ms = Some(now_ms);
        t.last_mission_id = Some(mission_id.to_string());
        match t.recurrence.interval()? {
            Some(interval) => t.next_fire_ms = now_ms + interval,
            None => t.enabled = false,
        }
        self.save(&t)?;
        Ok(t)
    }

    /// Due tasks turned into the mission-creation requests the caller
    /// should act on. Firing bookkeeping (`fire`) is the caller's
    /// responsibility *after* each mission is actually created — this
    /// method itself never mutates state, so a caller that crashes midway
    /// just sees the same due tasks again next time, never loses one.
    pub fn due_requests(&self, now_ms: u64) -> AraResult<Vec<FireRequest>> {
        Ok(self
            .due(now_ms)?
            .into_iter()
            .map(|t| FireRequest {
                task_id: t.id,
                goal: t.goal,
                persona: t.persona,
                max_steps: t.max_steps,
                max_duration_ms: t.max_duration_ms,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, SchedulerStore) {
        let d = tempfile::tempdir().unwrap();
        let s = SchedulerStore::open(d.path()).unwrap();
        (d, s)
    }
    const H: u64 = 3_600_000;

    #[test]
    fn rejects_a_non_future_time_and_an_empty_goal() {
        let (_d, s) = store();
        assert!(s.create("g", "general", 10, H, 100, Recurrence::Once, 100).is_err());
        assert!(s.create("g", "general", 10, H, 50, Recurrence::Once, 100).is_err());
        assert!(s.create("  ", "general", 10, H, 200, Recurrence::Once, 100).is_err());
    }

    #[test]
    fn rejects_a_repeat_interval_below_the_floor() {
        let (_d, s) = store();
        assert!(s
            .create("g", "general", 10, H, 200, Recurrence::Repeating(MIN_INTERVAL_MS - 1), 100)
            .is_err());
        assert!(s
            .create("g", "general", 10, H, 200, Recurrence::Repeating(MIN_INTERVAL_MS), 100)
            .is_ok());
    }

    #[test]
    fn a_once_task_fires_then_disables_itself() {
        let (_d, s) = store();
        let t = s.create("buy milk", "general", 10, H, 1_000, Recurrence::Once, 0).unwrap();
        assert!(s.due(500).unwrap().is_empty(), "not due yet");
        let due = s.due(1_000).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, t.id);
        let fired = s.fire(&t.id, 1_000, "mission-1").unwrap();
        assert!(!fired.enabled, "one-shot tasks disable after firing");
        assert_eq!(fired.last_mission_id.as_deref(), Some("mission-1"));
        assert!(s.due(2_000_000).unwrap().is_empty(), "disabled tasks never come due again");
    }

    #[test]
    fn a_repeating_task_advances_from_now_not_from_the_missed_slot() {
        let (_d, s) = store();
        let t = s
            .create("check inbox", "general", 10, H, 1_000, Recurrence::Repeating(MIN_INTERVAL_MS), 0)
            .unwrap();
        // Device was "off" for way more than one interval; fire() is called
        // very late relative to the original next_fire_ms.
        let late_now = 1_000 + MIN_INTERVAL_MS * 10;
        let fired = s.fire(&t.id, late_now, "mission-1").unwrap();
        assert!(fired.enabled, "repeating tasks stay enabled");
        assert_eq!(
            fired.next_fire_ms,
            late_now + MIN_INTERVAL_MS,
            "catches up to now, doesn't queue up 10 missed firings"
        );
    }

    #[test]
    fn due_requests_carries_the_mission_parameters_and_never_mutates() {
        let (_d, s) = store();
        s.create("اعمل تقرير", "analyst", 50, H, 1_000, Recurrence::Once, 0).unwrap();
        let r1 = s.due_requests(1_000).unwrap();
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].goal, "اعمل تقرير");
        assert_eq!(r1[0].persona, "analyst");
        // Calling it again without `fire` must see the same task again —
        // a caller that crashes between "ask what's due" and "create the
        // mission" must not have silently lost the task.
        let r2 = s.due_requests(1_000).unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn disabled_tasks_are_never_due_and_can_be_re_enabled() {
        let (_d, s) = store();
        let t = s.create("g", "general", 10, H, 1_000, Recurrence::Once, 0).unwrap();
        s.set_enabled(&t.id, false).unwrap();
        assert!(s.due(1_000).unwrap().is_empty());
        s.set_enabled(&t.id, true).unwrap();
        assert_eq!(s.due(1_000).unwrap().len(), 1);
    }

    #[test]
    fn delete_is_idempotent_and_list_is_ordered_by_next_fire() {
        let (_d, s) = store();
        let a = s.create("a", "general", 10, H, 3_000, Recurrence::Once, 0).unwrap();
        let b = s.create("b", "general", 10, H, 1_000, Recurrence::Once, 0).unwrap();
        let ids: Vec<_> = s.list().unwrap().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![b.id.clone(), a.id.clone()], "earliest next_fire_ms first");
        s.delete(&a.id).unwrap();
        s.delete(&a.id).unwrap(); // second delete of the same id is a no-op, not an error
        assert_eq!(s.list().unwrap().len(), 1);
    }

    #[test]
    fn rejects_path_traversal_ids() {
        let (_d, s) = store();
        assert!(s.get("../../etc/passwd").is_err());
        assert!(s.delete("../x").is_err());
    }
}
