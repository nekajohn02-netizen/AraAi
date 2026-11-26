//! Missions: tasks ARA works on by itself for minutes to 24 hours.
//!
//! Safety model (enforced here, not by the model):
//! * hard budget: max steps, max wall-clock (<= 24 h), checked every step;
//! * anything with an external effect becomes a pending [`Approval`] in the
//!   inbox — the mission keeps working on other things, never auto-approves,
//!   and approvals expire;
//! * every step and decision is appended to an audit log;
//! * state is saved atomically after each step, so a crash/reboot resumes
//!   from the last checkpoint; the user can pause, cancel or stop any time.
//!
//! The model-facing step is injected as a closure so the state machine is
//! fully testable without a model.

use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MAX_DURATION_MS: u64 = 24 * 3600 * 1000;
pub const MAX_STEPS_LIMIT: u32 = 2000;
pub const APPROVAL_TTL_MS: u64 = 12 * 3600 * 1000;
pub const MAX_PENDING: usize = 20;
/// A transient failure (a flaky tool call, a model hiccup) no longer kills
/// an hours-long mission outright: it gets this many retries first. Each
/// retry still counts as a normal step against the mission's own budget, so
/// a stuck mission can't loop forever — it just gets a few tries before
/// `Failed` becomes final.
pub const MAX_AUTO_RETRIES: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    /// Time or step budget exhausted.
    BudgetExhausted,
}

impl Status {
    pub fn is_final(self) -> bool {
        !matches!(self, Status::Running | Status::Paused)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub max_steps: u32,
    pub max_duration_ms: u64,
}

impl Budget {
    pub fn new(max_steps: u32, max_duration_ms: u64) -> AraResult<Self> {
        if max_steps == 0
            || max_steps > MAX_STEPS_LIMIT
            || max_duration_ms == 0
            || max_duration_ms > MAX_DURATION_MS
        {
            return Err(AraError::InvalidInput(format!(
                "budget must be 1..={MAX_STEPS_LIMIT} steps and at most 24 h"
            )));
        }
        Ok(Self {
            max_steps,
            max_duration_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    pub id: u32,
    /// Human-readable description of what will happen.
    pub summary: String,
    /// Opaque tool call to run after approval.
    pub call: serde_json::Value,
    pub requested_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepRecord {
    pub n: u32,
    pub at_ms: u64,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mission {
    pub id: String,
    pub goal: String,
    /// Persona id (specialist mode) the mission runs in.
    pub persona: String,
    pub status: Status,
    pub budget: Budget,
    pub created_ms: u64,
    pub steps_done: u32,
    /// Latest progress summary shown to the user.
    pub progress: String,
    pub pending: Vec<Approval>,
    pub next_approval_id: u32,
    /// Last few step notes (full history is in the audit log).
    pub recent: Vec<StepRecord>,
    pub result: Option<String>,
    /// Upfront task breakdown (Planning Engine), when the runner provided
    /// one via `StepOutcome::Plan`. Empty for missions that never planned
    /// explicitly — older saved missions load fine with no plan (`default`).
    #[serde(default)]
    pub plan: Vec<PlanStep>,
    /// Failures since the last non-`Failed` outcome; reset on any progress.
    /// Not shown to the user directly — `recent`'s notes already explain a
    /// retry in progress.
    #[serde(default)]
    pub consecutive_failures: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    pub description: String,
    pub done: bool,
}

/// What one model step produced.
#[derive(Debug, Clone, PartialEq)]
pub enum StepOutcome {
    /// Work continues; `note` is a one-line progress update.
    Progress {
        note: String,
    },
    /// The step wants an external action; needs the user's approval.
    NeedsApproval {
        summary: String,
        call: serde_json::Value,
    },
    /// Goal reached.
    Done {
        result: String,
    },
    Failed {
        reason: String,
    },
    /// Upfront task breakdown (Planning Engine): replaces the mission's
    /// `plan` wholesale. Doesn't have to be the first step — a mission can
    /// also replan mid-way after learning something that changes the task
    /// list — but produces no other side effect on its own.
    Plan {
        steps: Vec<String>,
    },
}

pub struct MissionStore {
    dir: PathBuf,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

impl MissionStore {
    pub fn open(dir: impl Into<PathBuf>) -> AraResult<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }
    fn audit_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.audit.jsonl"))
    }

    fn save(&self, m: &Mission) -> AraResult<()> {
        let tmp = self.dir.join(format!("{}.tmp", m.id));
        fs::write(
            &tmp,
            serde_json::to_vec(m).map_err(|e| AraError::Internal(e.to_string()))?,
        )?;
        fs::rename(&tmp, self.path(&m.id))?;
        Ok(())
    }

    /// Append-only audit trail (JSON lines).
    pub fn audit(&self, id: &str, now_ms: u64, event: &str, detail: &str) -> AraResult<()> {
        if !valid_id(id) {
            return Err(AraError::InvalidInput("bad mission id".into()));
        }
        let line = serde_json::json!({ "t": now_ms, "event": event, "detail": detail.chars().take(500).collect::<String>() });
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.audit_path(id))?;
        writeln!(f, "{line}")?;
        Ok(())
    }

    pub fn audit_log(&self, id: &str) -> AraResult<Vec<serde_json::Value>> {
        if !valid_id(id) {
            return Err(AraError::InvalidInput("bad mission id".into()));
        }
        Ok(fs::read_to_string(self.audit_path(id))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }

    pub fn create(
        &self,
        goal: &str,
        persona: &str,
        budget: Budget,
        now_ms: u64,
    ) -> AraResult<Mission> {
        let goal: String = goal
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(2000)
            .collect();
        if goal.trim().is_empty() {
            return Err(AraError::InvalidInput("empty goal".into()));
        }
        let id = format!("{now_ms:x}-{}", self.list()?.len());
        let m = Mission {
            id: id.clone(),
            goal,
            persona: persona.to_string(),
            status: Status::Running,
            budget,
            created_ms: now_ms,
            steps_done: 0,
            progress: String::new(),
            pending: vec![],
            next_approval_id: 1,
            recent: vec![],
            result: None,
            plan: vec![],
            consecutive_failures: 0,
        };
        self.save(&m)?;
        self.audit(&id, now_ms, "created", &m.goal)?;
        Ok(m)
    }

    pub fn get(&self, id: &str) -> AraResult<Mission> {
        if !valid_id(id) {
            return Err(AraError::InvalidInput("bad mission id".into()));
        }
        let b = fs::read(self.path(id)).map_err(|_| AraError::NotFound("mission".into()))?;
        serde_json::from_slice(&b)
            .map_err(|_| AraError::InvalidInput("mission file is corrupt".into()))
    }

    pub fn list(&self) -> AraResult<Vec<Mission>> {
        let mut v: Vec<Mission> = fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.ends_with(".json") && !n.ends_with(".audit.jsonl")
            })
            .filter_map(|e| fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice(&b).ok())
            .collect();
        v.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then(a.id.cmp(&b.id)));
        Ok(v)
    }

    pub fn set_status(&self, id: &str, status: Status, now_ms: u64) -> AraResult<Mission> {
        let mut m = self.get(id)?;
        if m.status.is_final() {
            return Err(AraError::InvalidInput("mission already finished".into()));
        }
        if !matches!(status, Status::Paused | Status::Running | Status::Cancelled) {
            return Err(AraError::InvalidInput(
                "user can only pause, resume or cancel".into(),
            ));
        }
        m.status = status;
        self.save(&m)?;
        self.audit(id, now_ms, "user_set_status", &format!("{status:?}"))?;
        Ok(m)
    }

    /// Resolves a pending approval. Returns the call to execute when approved.
    pub fn decide(
        &self,
        id: &str,
        approval_id: u32,
        approve: bool,
        now_ms: u64,
    ) -> AraResult<Option<serde_json::Value>> {
        let mut m = self.get(id)?;
        let pos = m
            .pending
            .iter()
            .position(|a| a.id == approval_id)
            .ok_or_else(|| AraError::NotFound("approval (maybe expired)".into()))?;
        let a = m.pending.remove(pos);
        if now_ms.saturating_sub(a.requested_ms) > APPROVAL_TTL_MS {
            self.save(&m)?;
            self.audit(id, now_ms, "approval_expired", &a.summary)?;
            return Err(AraError::InvalidInput("approval expired".into()));
        }
        self.save(&m)?;
        self.audit(
            id,
            now_ms,
            if approve { "approved" } else { "declined" },
            &a.summary,
        )?;
        Ok(approve.then_some(a.call))
    }

    /// Runs ONE step if the mission is runnable. Idempotent per call: the
    /// checkpoint is saved before returning.
    pub fn step(
        &self,
        id: &str,
        now_ms: u64,
        run: &mut dyn FnMut(&Mission) -> StepOutcome,
    ) -> AraResult<Mission> {
        let mut m = self.get(id)?;
        if m.status != Status::Running {
            return Ok(m);
        }
        // Expire stale approvals.
        let before = m.pending.len();
        m.pending
            .retain(|a| now_ms.saturating_sub(a.requested_ms) <= APPROVAL_TTL_MS);
        if m.pending.len() != before {
            self.audit(
                id,
                now_ms,
                "approvals_expired",
                &(before - m.pending.len()).to_string(),
            )?;
        }
        // Budget is checked before doing any work.
        if now_ms.saturating_sub(m.created_ms) >= m.budget.max_duration_ms
            || m.steps_done >= m.budget.max_steps
        {
            m.status = Status::BudgetExhausted;
            self.save(&m)?;
            self.audit(
                id,
                now_ms,
                "budget_exhausted",
                &format!("steps={}", m.steps_done),
            )?;
            return Ok(m);
        }
        // Too many unanswered approvals: wait for the user instead of piling up.
        if m.pending.len() >= MAX_PENDING {
            self.save(&m)?;
            return Ok(m);
        }
        m.steps_done += 1;
        let outcome = run(&m);
        if !matches!(outcome, StepOutcome::Failed { .. }) {
            m.consecutive_failures = 0;
        }
        let note = match &outcome {
            StepOutcome::Progress { note } => note.clone(),
            StepOutcome::NeedsApproval { summary, call } => {
                let a = Approval {
                    id: m.next_approval_id,
                    summary: summary.clone(),
                    call: call.clone(),
                    requested_ms: now_ms,
                };
                m.next_approval_id += 1;
                self.audit(id, now_ms, "approval_requested", summary)?;
                m.pending.push(a);
                format!("waiting for approval: {summary}")
            }
            StepOutcome::Done { result } => {
                m.status = Status::Completed;
                m.result = Some(result.clone());
                result.clone()
            }
            StepOutcome::Plan { steps } => {
                m.plan = steps
                    .iter()
                    .map(|s| PlanStep {
                        description: s.chars().take(300).collect(),
                        done: false,
                    })
                    .collect();
                self.audit(id, now_ms, "planned", &format!("{} steps", m.plan.len()))?;
                format!("planned {} step(s)", m.plan.len())
            }
            StepOutcome::Failed { reason } => {
                m.consecutive_failures += 1;
                if m.consecutive_failures > MAX_AUTO_RETRIES {
                    m.status = Status::Failed;
                    reason.clone()
                } else {
                    self.audit(
                        id,
                        now_ms,
                        "retrying_after_failure",
                        &format!("attempt {}/{MAX_AUTO_RETRIES}: {reason}", m.consecutive_failures),
                    )?;
                    format!(
                        "retrying after a failure ({reason}) — attempt {}/{MAX_AUTO_RETRIES}",
                        m.consecutive_failures
                    )
                }
            }
        };
        m.progress = note.chars().take(300).collect();
        m.recent.push(StepRecord {
            n: m.steps_done,
            at_ms: now_ms,
            note: m.progress.clone(),
        });
        if m.recent.len() > 20 {
            m.recent.remove(0);
        }
        self.save(&m)?;
        self.audit(id, now_ms, "step", &m.progress)?;
        Ok(m)
    }

    /// Recovery after a restart: missions that were Running continue; nothing
    /// is lost because every step is checkpointed.
    pub fn resumable(&self) -> AraResult<Vec<String>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|m| m.status == Status::Running)
            .map(|m| m.id)
            .collect())
    }
}

pub fn exists(dir: &Path) -> bool {
    dir.is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store() -> (tempfile::TempDir, MissionStore) {
        let d = tempfile::tempdir().unwrap();
        let s = MissionStore::open(d.path()).unwrap();
        (d, s)
    }
    const H: u64 = 3_600_000;

    #[test]
    fn runs_to_completion_with_audit_trail() {
        let (_d, s) = store();
        let m = s
            .create(
                "اعمل ملخص للملفات",
                "analyst",
                Budget::new(10, H).unwrap(),
                1000,
            )
            .unwrap();
        let mut n = 0;
        let mut run = |_: &Mission| {
            n += 1;
            if n < 3 {
                StepOutcome::Progress {
                    note: format!("part {n}"),
                }
            } else {
                StepOutcome::Done {
                    result: "تم".into(),
                }
            }
        };
        for t in 0..5 {
            s.step(&m.id, 2000 + t, &mut run).unwrap();
        }
        let f = s.get(&m.id).unwrap();
        assert_eq!(f.status, Status::Completed);
        assert_eq!(f.steps_done, 3, "no steps after completion");
        assert_eq!(f.result.as_deref(), Some("تم"));
        let log = s.audit_log(&m.id).unwrap();
        assert_eq!(log.first().unwrap()["event"], "created");
        assert_eq!(log.iter().filter(|e| e["event"] == "step").count(), 3);
    }

    #[test]
    fn budget_caps_steps_and_time() {
        let (_d, s) = store();
        let m = s
            .create("x", "general", Budget::new(2, H).unwrap(), 0)
            .unwrap();
        let mut run = |_: &Mission| StepOutcome::Progress { note: "w".into() };
        for _ in 0..5 {
            s.step(&m.id, 10, &mut run).unwrap();
        }
        assert_eq!(s.get(&m.id).unwrap().status, Status::BudgetExhausted);
        assert_eq!(s.get(&m.id).unwrap().steps_done, 2);
        let m2 = s
            .create("y", "general", Budget::new(100, H).unwrap(), 0)
            .unwrap();
        s.step(&m2.id, H + 1, &mut run).unwrap();
        assert_eq!(s.get(&m2.id).unwrap().status, Status::BudgetExhausted);
        assert!(Budget::new(10, MAX_DURATION_MS + 1).is_err());
        assert!(Budget::new(0, H).is_err());
    }

    #[test]
    fn external_actions_wait_for_user_and_expire() {
        let (_d, s) = store();
        let m = s
            .create("ابعت تقرير", "planner", Budget::new(50, 24 * H).unwrap(), 0)
            .unwrap();
        let call = json!({"tool":"whatsapp_compose","arguments":{"text":"hi"}});
        let mut first = true;
        let mut run = |_: &Mission| {
            if std::mem::take(&mut first) {
                StepOutcome::NeedsApproval {
                    summary: "WhatsApp → hi".into(),
                    call: call.clone(),
                }
            } else {
                StepOutcome::Progress {
                    note: "kept working".into(),
                }
            }
        };
        let after = s.step(&m.id, 100, &mut run).unwrap();
        assert_eq!(after.pending.len(), 1);
        // The mission continues with other work while the approval waits.
        let after = s.step(&m.id, 200, &mut run).unwrap();
        assert_eq!(after.progress, "kept working");
        // Approval is executed only on explicit approve, exactly once.
        let got = s.decide(&m.id, 1, true, 300).unwrap().unwrap();
        assert_eq!(got["tool"], "whatsapp_compose");
        assert!(s.decide(&m.id, 1, true, 300).is_err(), "single use");
        // Declined → nothing to execute. Expired → error.
        let mut r2 = |_: &Mission| StepOutcome::NeedsApproval {
            summary: "s".into(),
            call: json!({}),
        };
        s.step(&m.id, 400, &mut r2).unwrap();
        assert!(s.decide(&m.id, 2, false, 500).unwrap().is_none());
        s.step(&m.id, 600, &mut r2).unwrap();
        assert!(s.decide(&m.id, 3, true, 600 + APPROVAL_TTL_MS + 1).is_err());
        assert!(s
            .audit_log(&m.id)
            .unwrap()
            .iter()
            .any(|e| e["event"] == "approval_expired"));
    }

    #[test]
    fn pause_cancel_and_resume_after_restart() {
        let d = tempfile::tempdir().unwrap();
        let id = {
            let s = MissionStore::open(d.path()).unwrap();
            let m = s
                .create("long job", "general", Budget::new(10, H).unwrap(), 0)
                .unwrap();
            s.step(&m.id, 1, &mut |_| StepOutcome::Progress {
                note: "a".into(),
            })
            .unwrap();
            s.set_status(&m.id, Status::Paused, 2).unwrap();
            let mut called = false;
            s.step(&m.id, 3, &mut |_| {
                called = true;
                StepOutcome::Progress { note: "b".into() }
            })
            .unwrap();
            assert!(!called, "paused missions do not run");
            s.set_status(&m.id, Status::Running, 4).unwrap();
            m.id
        };
        // "Reboot": new store over the same directory.
        let s = MissionStore::open(d.path()).unwrap();
        assert_eq!(s.resumable().unwrap(), vec![id.clone()]);
        assert_eq!(s.get(&id).unwrap().steps_done, 1);
        s.set_status(&id, Status::Cancelled, 5).unwrap();
        assert!(
            s.set_status(&id, Status::Running, 6).is_err(),
            "final is final"
        );
        assert!(s.resumable().unwrap().is_empty());
        assert!(s.get("../x").is_err());
    }

    #[test]
    fn user_cannot_force_completed_and_repeated_failures_eventually_stop() {
        let (_d, s) = store();
        let m = s
            .create("g", "general", Budget::new(5, H).unwrap(), 0)
            .unwrap();
        assert!(s.set_status(&m.id, Status::Completed, 1).is_err());
        let mut fail = |_: &Mission| StepOutcome::Failed {
            reason: "model error".into(),
        };
        // A transient failure gets MAX_AUTO_RETRIES retries before the
        // mission actually stops — a single hiccup must not kill an
        // hours-long mission outright.
        for _ in 0..MAX_AUTO_RETRIES {
            let m = s.step(&m.id, 2, &mut fail).unwrap();
            assert_eq!(m.status, Status::Running, "still retrying");
        }
        let m = s.step(&m.id, 2, &mut fail).unwrap();
        assert_eq!(m.status, Status::Failed, "retries exhausted");
        assert!(s
            .create("   ", "general", Budget::new(5, H).unwrap(), 0)
            .is_err());
    }

    #[test]
    fn a_later_success_resets_the_failure_streak() {
        let (_d, s) = store();
        let m = s
            .create("g", "general", Budget::new(10, H).unwrap(), 0)
            .unwrap();
        let mut fail = |_: &Mission| StepOutcome::Failed { reason: "x".into() };
        s.step(&m.id, 2, &mut fail).unwrap();
        assert_eq!(s.get(&m.id).unwrap().consecutive_failures, 1);
        s.step(&m.id, 3, &mut |_| StepOutcome::Progress { note: "ok".into() })
            .unwrap();
        assert_eq!(s.get(&m.id).unwrap().consecutive_failures, 0, "progress resets the streak");
        // So the mission can now absorb MAX_AUTO_RETRIES more failures again.
        for _ in 0..MAX_AUTO_RETRIES {
            let m = s.step(&m.id, 4, &mut fail).unwrap();
            assert_eq!(m.status, Status::Running);
        }
    }

    #[test]
    fn plan_outcome_sets_the_task_breakdown() {
        let (_d, s) = store();
        let m = s
            .create("g", "general", Budget::new(5, H).unwrap(), 0)
            .unwrap();
        let f = s
            .step(&m.id, 1, &mut |_| StepOutcome::Plan {
                steps: vec!["ابحث عن الموضوع".into(), "اكتب الملخص".into()],
            })
            .unwrap();
        assert_eq!(f.plan.len(), 2);
        assert_eq!(f.plan[0].description, "ابحث عن الموضوع");
        assert!(!f.plan[0].done);
        assert_eq!(f.status, Status::Running, "planning alone doesn't finish the mission");
        // Replanning replaces the breakdown wholesale.
        let f2 = s
            .step(&m.id, 2, &mut |_| StepOutcome::Plan {
                steps: vec!["خطة جديدة".into()],
            })
            .unwrap();
        assert_eq!(f2.plan.len(), 1);
    }
}
