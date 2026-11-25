//! Local, privacy-respecting activity log — the first real piece of the
//! Privacy Center (T1.7's "سجل لكل طلب") and of per-skill action tracking
//! (T2.2e, ADR-017). Deliberately metadata-only: which tool ran, which
//! skill(s) routed it, and the outcome — **never** the call's arguments or
//! output, which can carry the user's own message content, phone numbers,
//! file contents, and so on. This file answers "what kind of thing
//! happened, and was it allowed", not "what did I say" — that stays only
//! in the conversation it belongs to (and encrypted, if T1.8's key is in
//! use).
//!
//! Capped at `MAX_ENTRIES`, oldest dropped first — the same bounded-growth
//! discipline as `learned_aliases` (ADR-013's `MAX_LEARNED_ALIASES`): this
//! file can never grow without limit no matter how long the app is used.

use ara_core_api::AraResult;
use ara_tools::Dispatch;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub ts_ms: u64,
    pub conversation_id: Option<String>,
    pub mission_id: Option<String>,
    /// Skills routed for this turn (empty if none were, or not applicable).
    pub skill_ids: Vec<String>,
    pub tool: String,
    /// `"done" | "needs_confirmation" | "run_on_host" | "run_service" | "denied"`.
    pub outcome: String,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Pulls the two facts worth logging out of a `Dispatch` — never the
/// `output`/`arguments` payload inside it.
pub fn tool_and_outcome(dispatch: &Dispatch) -> (String, &'static str) {
    match dispatch {
        Dispatch::Done { tool, .. } => (tool.clone(), "done"),
        Dispatch::NeedsConfirmation { tool, .. } => (tool.clone(), "needs_confirmation"),
        Dispatch::RunOnHost { call, .. } => (call.tool.clone(), "run_on_host"),
        Dispatch::RunService { call } => (call.tool.clone(), "run_service"),
        Dispatch::Denied { tool, .. } => (tool.clone(), "denied"),
    }
}

pub struct ActivityLog {
    path: PathBuf,
}

impl ActivityLog {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn read_all(&self) -> Vec<ActivityEntry> {
        std::fs::read(&self.path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Appends one turn's worth of entries, oldest-first within the batch,
    /// then trims to `MAX_ENTRIES` (dropping the oldest overall). A no-op
    /// for an empty batch — most mission steps and plain chat turns with
    /// no tool use log nothing, which is correct, not a missing feature.
    pub fn append(&self, mut entries: Vec<ActivityEntry>) -> AraResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut all = self.read_all();
        all.append(&mut entries);
        if all.len() > MAX_ENTRIES {
            let excess = all.len() - MAX_ENTRIES;
            all.drain(0..excess);
        }
        let tmp = self.path.with_extension("tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec(&all).map_err(|e| ara_core_api::AraError::Internal(e.to_string()))?,
        )?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Newest first, capped at `limit`.
    pub fn list(&self, limit: usize) -> Vec<ActivityEntry> {
        let mut all = self.read_all();
        all.reverse();
        all.truncate(limit);
        all
    }

    pub fn entry(
        conversation_id: Option<String>,
        mission_id: Option<String>,
        skill_ids: Vec<String>,
        dispatch: &Dispatch,
    ) -> ActivityEntry {
        let (tool, outcome) = tool_and_outcome(dispatch);
        ActivityEntry {
            ts_ms: now_ms(),
            conversation_id,
            mission_id,
            skill_ids,
            tool,
            outcome: outcome.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ara_tools::ToolCall;

    fn done(tool: &str) -> Dispatch {
        Dispatch::Done {
            tool: tool.into(),
            output: serde_json::json!({"secret": "this must never be logged"}),
        }
    }

    #[test]
    fn append_and_list_round_trip_newest_first() {
        let d = tempfile::tempdir().unwrap();
        let log = ActivityLog::open(d.path().join("activity.json"));
        log.append(vec![ActivityLog::entry(
            Some("c1".into()),
            None,
            vec!["math".into()],
            &done("calculator"),
        )])
        .unwrap();
        log.append(vec![ActivityLog::entry(
            Some("c1".into()),
            None,
            vec!["messaging".into()],
            &done("whatsapp_compose"),
        )])
        .unwrap();

        let got = log.list(10);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].tool, "whatsapp_compose", "newest first");
        assert_eq!(got[1].tool, "calculator");
    }

    #[test]
    fn empty_batch_is_a_no_op() {
        let d = tempfile::tempdir().unwrap();
        let log = ActivityLog::open(d.path().join("activity.json"));
        log.append(vec![]).unwrap();
        assert_eq!(log.list(10).len(), 0);
    }

    #[test]
    fn outcome_and_tool_are_extracted_for_every_dispatch_variant() {
        assert_eq!(
            tool_and_outcome(&Dispatch::Done {
                tool: "calculator".into(),
                output: serde_json::json!(null)
            }),
            ("calculator".to_string(), "done")
        );
        assert_eq!(
            tool_and_outcome(&Dispatch::NeedsConfirmation {
                tool: "whatsapp_compose".into(),
                summary: "x".into(),
                call: ToolCall {
                    tool: "whatsapp_compose".into(),
                    arguments: serde_json::json!({})
                }
            }),
            ("whatsapp_compose".to_string(), "needs_confirmation")
        );
        assert_eq!(
            tool_and_outcome(&Dispatch::Denied {
                tool: "run_shell".into(),
                reason: "unknown tool".into()
            }),
            ("run_shell".to_string(), "denied")
        );
    }

    #[test]
    fn log_never_contains_the_call_arguments_or_output() {
        let d = tempfile::tempdir().unwrap();
        let log = ActivityLog::open(d.path().join("activity.json"));
        log.append(vec![ActivityLog::entry(
            Some("c1".into()),
            None,
            vec![],
            &done("calculator"),
        )])
        .unwrap();

        let raw = std::fs::read_to_string(d.path().join("activity.json")).unwrap();
        assert!(
            !raw.contains("secret"),
            "the log file must never contain tool output/argument content"
        );
    }

    #[test]
    fn caps_at_max_entries_dropping_the_oldest() {
        let d = tempfile::tempdir().unwrap();
        let log = ActivityLog::open(d.path().join("activity.json"));
        for i in 0..(MAX_ENTRIES + 10) {
            log.append(vec![ActivityLog::entry(
                None,
                None,
                vec![],
                &done(&format!("tool-{i}")),
            )])
            .unwrap();
        }
        let all = log.list(MAX_ENTRIES + 10);
        assert_eq!(all.len(), MAX_ENTRIES);
        assert_eq!(all[0].tool, format!("tool-{}", MAX_ENTRIES + 9), "newest kept");
        assert_eq!(all[MAX_ENTRIES - 1].tool, "tool-10", "oldest 10 were dropped");
    }
}
