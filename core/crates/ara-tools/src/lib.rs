//! Tools and skills.
//!
//! * A **tool** is one typed action (calculator, set an alarm, draft a
//!   WhatsApp message …) described by a [`ToolManifest`].
//! * A **skill** bundles instructions + a set of tools ([`skills`]).
//! * Every call coming from a model is **untrusted**: [`ToolRegistry::dispatch`]
//!   parses, validates arguments against the schema, checks the user's
//!   permission grants, and requires explicit confirmation for anything with
//!   side effects — before any executor runs.
//! * `Core` tools run in Rust ([`builtin`]); `Host` tools (Android intents,
//!   alarms, sharing) are returned to the app to execute after these checks.
//!
//! There is deliberately no shell / arbitrary-code tool.

pub mod builtin;
pub mod parse;
pub mod personas;
pub mod policy;
pub mod schema;
pub mod skills;

use ara_core_api::{AraError, AraResult};
use policy::{Decision, PermissionStore};
use schema::ParamSpec;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Capabilities a tool may need. Granted per tool by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Open another app or a system screen.
    LaunchApps,
    /// Hand text to a messaging app (WhatsApp, Telegram, SMS …) for the user to send.
    ComposeMessages,
    /// Share text to any app chosen by the user (social networks, mail …).
    ShareContent,
    PhoneDialer,
    Alarms,
    Calendar,
    Clipboard,
    /// Torch, volume, Wi-Fi/Bluetooth settings screens.
    DeviceControls,
    /// Open web pages in the browser.
    OpenWeb,
    /// Read files the user picked.
    ReadUserFiles,
    /// Save facts to ARA's long-term memory.
    Memory,
}

/// What happens to the world when the tool runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    /// Pure computation / reading. No confirmation needed.
    None,
    /// Changes local state the user can undo (alarm, clipboard, opens an app).
    Reversible,
    /// Reaches other people or services (messages, posts, calls).
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    /// Executed by the Rust core.
    Core,
    /// Executed by the Android app after the core approved it.
    Host,
    /// Executed by the embedding Rust layer that owns state (search index,
    /// memory) through [`Services`], after the same checks.
    Service,
}

/// Stateful tool executor supplied by the embedding layer (e.g. AraCore).
pub trait Services {
    fn run(&self, call: &ToolCall) -> AraResult<Value>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolManifest {
    /// Stable id `[a-z0-9_]+`, used by the model.
    pub id: String,
    pub version: u32,
    /// One line, shown to the model and to the user.
    pub description: String,
    pub params: BTreeMap<String, ParamSpec>,
    pub permissions: Vec<Permission>,
    pub side_effect: SideEffect,
    pub runtime: Runtime,
    pub timeout_ms: u32,
    pub max_output_bytes: u32,
    /// Max calls per minute (abuse / loop protection).
    pub rate_per_minute: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub arguments: Value,
}

/// Outcome of dispatching one call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Dispatch {
    /// Core tool ran; `output` is data for the model (untrusted on return).
    Done {
        tool: String,
        output: Value,
    },
    /// The app must ask the user; re-dispatch with `confirmed = true`.
    NeedsConfirmation {
        tool: String,
        summary: String,
        call: ToolCall,
    },
    /// Approved host tool: the app executes it now.
    RunOnHost {
        call: ToolCall,
        timeout_ms: u32,
    },
    /// Approved service tool: the embedding layer runs it via [`Services`].
    RunService {
        call: ToolCall,
    },
    Denied {
        tool: String,
        reason: String,
    },
}

pub struct ToolRegistry {
    tools: BTreeMap<String, ToolManifest>,
    rate: std::sync::Mutex<BTreeMap<String, Vec<std::time::Instant>>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl ToolRegistry {
    pub fn empty() -> Self {
        Self {
            tools: BTreeMap::new(),
            rate: Default::default(),
        }
    }

    /// Registry with all built-in core and host tools.
    pub fn with_builtins() -> Self {
        let mut r = Self::empty();
        for m in builtin::core_manifests()
            .into_iter()
            .chain(builtin::host_manifests())
            .chain(builtin::service_manifests())
        {
            r.register(m).expect("built-in manifests are valid");
        }
        r
    }

    pub fn register(&mut self, m: ToolManifest) -> AraResult<()> {
        if m.id.is_empty()
            || !m
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(AraError::InvalidInput(format!("bad tool id '{}'", m.id)));
        }
        if self.tools.contains_key(&m.id) {
            return Err(AraError::InvalidInput(format!("duplicate tool '{}'", m.id)));
        }
        if m.side_effect != SideEffect::None && m.permissions.is_empty() {
            return Err(AraError::InvalidInput(format!(
                "tool '{}' has side effects but declares no permission",
                m.id
            )));
        }
        if m.timeout_ms == 0 || m.max_output_bytes == 0 || m.rate_per_minute == 0 {
            return Err(AraError::InvalidInput(format!(
                "tool '{}' needs limits",
                m.id
            )));
        }
        self.tools.insert(m.id.clone(), m);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ToolManifest> {
        self.tools.get(id)
    }

    pub fn manifests(&self) -> impl Iterator<Item = &ToolManifest> {
        self.tools.values()
    }

    /// Compact JSON description of `ids` for the model prompt.
    pub fn describe_for_model(&self, ids: &[&str]) -> Value {
        Value::Array(
            ids.iter()
                .filter_map(|id| self.tools.get(*id))
                .map(|m| {
                    serde_json::json!({
                        "name": m.id,
                        "description": m.description,
                        "parameters": schema::to_json_schema(&m.params),
                    })
                })
                .collect(),
        )
    }

    fn rate_ok(&self, m: &ToolManifest) -> bool {
        let now = std::time::Instant::now();
        let mut map = self.rate.lock().unwrap_or_else(|e| e.into_inner());
        let v = map.entry(m.id.clone()).or_default();
        v.retain(|t| now.duration_since(*t).as_secs() < 60);
        if v.len() as u32 >= m.rate_per_minute {
            return false;
        }
        v.push(now);
        true
    }

    /// The single entry point for executing model-requested actions.
    pub fn dispatch(&self, call: &ToolCall, grants: &PermissionStore, confirmed: bool) -> Dispatch {
        let deny = |reason: String| Dispatch::Denied {
            tool: call.tool.clone(),
            reason,
        };
        let Some(m) = self.tools.get(&call.tool) else {
            return deny("unknown tool".into());
        };
        let args = match schema::validate(&m.params, &call.arguments) {
            Ok(a) => a,
            Err(e) => return deny(format!("invalid arguments: {e}")),
        };
        if let Err(e) = builtin::precheck(&m.id, &args) {
            return deny(format!("invalid arguments: {e}"));
        }
        match grants.decide(m) {
            Decision::Deny(r) => return deny(r),
            Decision::Allow => {}
            Decision::Ask if !confirmed => {
                return Dispatch::NeedsConfirmation {
                    tool: m.id.clone(),
                    summary: builtin::summarize(m, &args),
                    call: ToolCall {
                        tool: m.id.clone(),
                        arguments: args,
                    },
                }
            }
            Decision::Ask => {}
        }
        if !self.rate_ok(m) {
            return deny("rate limit exceeded".into());
        }
        let call = ToolCall {
            tool: m.id.clone(),
            arguments: args,
        };
        match m.runtime {
            Runtime::Service => Dispatch::RunService { call },
            Runtime::Host => Dispatch::RunOnHost {
                call,
                timeout_ms: m.timeout_ms,
            },
            Runtime::Core => match builtin::run_core(&call) {
                Ok(out) => {
                    let s = out.to_string();
                    if s.len() > m.max_output_bytes as usize {
                        deny("output too large".into())
                    } else {
                        Dispatch::Done {
                            tool: m.id.clone(),
                            output: out,
                        }
                    }
                }
                Err(e) => deny(e.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use policy::Grant;
    use serde_json::json;

    fn call(tool: &str, args: Value) -> ToolCall {
        ToolCall {
            tool: tool.into(),
            arguments: args,
        }
    }

    #[test]
    fn builtins_register_and_have_sane_limits() {
        let r = ToolRegistry::with_builtins();
        assert!(r.manifests().count() >= 15);
        for m in r.manifests() {
            assert!(m.description.len() < 200, "{}", m.id);
        }
    }

    #[test]
    fn read_only_core_tool_runs_without_confirmation() {
        let r = ToolRegistry::with_builtins();
        let d = r.dispatch(
            &call("calculator", json!({"expression": "2*(3+4)"})),
            &PermissionStore::default(),
            false,
        );
        assert_eq!(
            d,
            Dispatch::Done {
                tool: "calculator".into(),
                output: json!({"result": 14.0})
            }
        );
    }

    #[test]
    fn external_tool_needs_permission_then_confirmation() {
        let r = ToolRegistry::with_builtins();
        let c = call(
            "whatsapp_compose",
            json!({"text": "أهلا", "phone": "+201000000000"}),
        );
        // Not granted yet.
        assert!(matches!(
            r.dispatch(&c, &PermissionStore::default(), true),
            Dispatch::Denied { .. }
        ));
        let mut g = PermissionStore::default();
        g.set(Permission::ComposeMessages, Grant::AskEveryTime);
        let d = r.dispatch(&c, &g, false);
        let Dispatch::NeedsConfirmation { summary, .. } = d else {
            panic!("{d:?}")
        };
        assert!(summary.contains("WhatsApp") && summary.contains("أهلا"));
        assert!(matches!(
            r.dispatch(&c, &g, true),
            Dispatch::RunOnHost { .. }
        ));
    }

    #[test]
    fn external_tools_always_confirm_even_if_always_allowed() {
        let r = ToolRegistry::with_builtins();
        let mut g = PermissionStore::default();
        g.set(Permission::ComposeMessages, Grant::Always);
        let d = r.dispatch(&call("telegram_compose", json!({"text": "hi"})), &g, false);
        assert!(matches!(d, Dispatch::NeedsConfirmation { .. }), "{d:?}");
    }

    #[test]
    fn invalid_or_unknown_calls_are_denied() {
        let r = ToolRegistry::with_builtins();
        let g = PermissionStore::allow_all_for_tests();
        for c in [
            call("shell", json!({"cmd": "rm -rf /"})),
            call("calculator", json!({})),
            call("calculator", json!({"expression": 5})),
            call("set_alarm", json!({"hour": 25, "minute": 0})),
            call("calculator", json!({"expression": "1", "extra": true})),
        ] {
            assert!(
                matches!(r.dispatch(&c, &g, true), Dispatch::Denied { .. }),
                "{c:?}"
            );
        }
    }

    #[test]
    fn rate_limit_stops_runaway_loops() {
        let r = ToolRegistry::with_builtins();
        let g = PermissionStore::default();
        let c = call("calculator", json!({"expression": "1+1"}));
        let limit = r.get("calculator").unwrap().rate_per_minute;
        for _ in 0..limit {
            assert!(matches!(r.dispatch(&c, &g, false), Dispatch::Done { .. }));
        }
        assert!(matches!(r.dispatch(&c, &g, false), Dispatch::Denied { .. }));
    }

    #[test]
    fn registry_rejects_unsafe_manifests() {
        let mut r = ToolRegistry::empty();
        let mut m = builtin::core_manifests().remove(0);
        m.side_effect = SideEffect::External;
        m.permissions.clear();
        assert!(r.register(m.clone()).is_err());
        m.id = "Bad-Id".into();
        assert!(r.register(m).is_err());
    }
}
