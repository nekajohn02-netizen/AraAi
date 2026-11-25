//! Stateful core object for the app. All methods are blocking: Kotlin calls
//! them from background dispatchers and receives streaming callbacks.
//!
//! Data layout under `data_dir` (app-private storage):
//!   models/            GGUF files (see ara-model-manager)
//!   conversations/     one JSON per conversation
//!   benchmarks.json    measurements on this device
//!   permissions.json   user grants per permission
//!   settings.json      storage cap etc.
//!   index.db           SQLite: document search index + user memories
//!   providers.json     API Hub providers (never the API keys)

use crate::conversations::ConversationStore;
use crate::{parse, to_json, FfiError};
use ara_coordinator::{Coordinator, IndexServices};
use ara_core_api::{AraError, CancellationToken};
use ara_device_profiler::HostHints;
use ara_inference::llama::LlamaEngine;
use ara_inference::{GenerationParams, InferenceEngine, LoadParams, LoadedModel};
use ara_model_manager::{InstallState, ModelStore, StoragePolicy};
use ara_model_registry::{Capability, ModelRegistry};
use ara_model_selector::{Benchmark, ModelSelector, SelectionMode, SelectionRequest};
use ara_search::SearchIndex;
use ara_tools::policy::{Grant, PermissionStore};
use ara_tools::skills::{self, SkillManifest};
use ara_tools::{Dispatch, Permission, Services, ToolCall, ToolRegistry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

const CATALOG: &str = include_str!("../../../catalog/models.json");

#[uniffi::export(callback_interface)]
pub trait TextListener: Send + Sync {
    fn on_text(&self, text: String);
}

#[uniffi::export(callback_interface)]
pub trait ProgressListener: Send + Sync {
    fn on_progress(&self, done: u64, total: u64);
}

/// Cancels a running download or generation from another thread.
#[derive(uniffi::Object, Default)]
pub struct CancelHandle(CancellationToken);

#[uniffi::export]
impl CancelHandle {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn cancel(&self) {
        self.0.cancel()
    }
}

/// A cloud provider configured by the user. The API key is NOT stored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    /// Redact personal data before sending (default on).
    #[serde(default = "yes")]
    pub shield: bool,
    /// The user explicitly agreed that messages (and file passages they ask
    /// about) are sent to this provider.
    #[serde(default)]
    pub consent: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Settings {
    /// Cap on total model storage, bytes. None = only keep 1 GiB free.
    max_model_storage_bytes: Option<u64>,
    /// Context window used when loading models (smaller = less RAM).
    #[serde(default)]
    context_tokens: Option<u32>,
}

struct Loaded {
    id: String,
    model: Box<dyn LoadedModel>,
}

#[derive(uniffi::Object)]
pub struct AraCore {
    home: PathBuf,
    registry: ModelRegistry,
    store: ModelStore,
    conversations: ConversationStore,
    tools: ToolRegistry,
    skills: Vec<SkillManifest>,
    grants: Mutex<PermissionStore>,
    settings: Mutex<Settings>,
    model: Mutex<Option<Loaded>>,
    index: Mutex<SearchIndex>,
    providers: Mutex<Vec<Provider>>,
    missions: ara_missions::MissionStore,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn read_json<T: serde::de::DeserializeOwned + Default>(p: &PathBuf) -> T {
    std::fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_json<T: Serialize>(p: &PathBuf, v: &T) -> Result<(), FfiError> {
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, to_json(v)?).map_err(AraError::from)?;
    std::fs::rename(&tmp, p).map_err(AraError::from)?;
    Ok(())
}

#[derive(Serialize)]
struct ModelRow<'a> {
    #[serde(flatten)]
    spec: &'a ara_model_registry::ModelSpec,
    install: InstallState,
    loaded: bool,
    /// Present when the model can run here.
    candidate: Option<&'a ara_model_selector::Candidate>,
    /// Present when it cannot.
    excluded: Option<&'a ara_model_selector::Exclusion>,
    recommended: bool,
}

impl AraCore {
    fn policy(&self) -> StoragePolicy {
        StoragePolicy {
            max_total_bytes: lock(&self.settings).max_model_storage_bytes,
            ..Default::default()
        }
    }
    fn verifications(&self) -> Vec<ara_coordinator::verify::VerifyReport> {
        read_json(&self.home.join("verification.json"))
    }
    fn failed_verification(&self) -> std::collections::BTreeMap<String, BTreeSet<Capability>> {
        let mut m = std::collections::BTreeMap::new();
        for r in self.verifications() {
            let mut f = BTreeSet::new();
            if !r.tools_ok() {
                f.insert(Capability::ToolUse);
            }
            if !f.is_empty() {
                m.insert(r.model_id, f);
            }
        }
        m
    }
    fn benchmarks(&self) -> Vec<Benchmark> {
        read_json(&self.home.join("benchmarks.json"))
    }
    fn spec(&self, id: &str) -> Result<&ara_model_registry::ModelSpec, FfiError> {
        self.registry
            .get(id)
            .ok_or_else(|| AraError::NotFound(format!("model '{id}'")).into())
    }
    fn context(&self) -> u32 {
        lock(&self.settings).context_tokens.unwrap_or(4096)
    }
}

#[uniffi::export]
impl AraCore {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Self>, FfiError> {
        let home = PathBuf::from(data_dir);
        std::fs::create_dir_all(&home).map_err(AraError::from)?;
        let tools = ToolRegistry::with_builtins();
        let skills = skills::builtin_skills();
        for s in &skills {
            skills::validate_skill(s, &tools)?;
        }
        Ok(Arc::new(Self {
            registry: ModelRegistry::from_json(CATALOG)?,
            store: ModelStore::open(home.join("models"))?,
            conversations: ConversationStore::open(home.join("conversations"))?,
            grants: Mutex::new(read_json(&home.join("permissions.json"))),
            settings: Mutex::new(read_json(&home.join("settings.json"))),
            model: Mutex::new(None),
            index: Mutex::new(SearchIndex::open(&home.join("index.db"))?),
            providers: Mutex::new(read_json(&home.join("providers.json"))),
            missions: ara_missions::MissionStore::open(home.join("missions"))?,
            tools,
            skills,
            home,
        }))
    }

    // ------------------------------------------------------------ device

    pub fn device_profile(&self, hints_json: String) -> Result<String, FfiError> {
        let hints: HostHints = parse("hints", &hints_json)?;
        to_json(&ara_device_profiler::collect(self.store.root(), hints)?)
    }

    /// All catalog models with install state and whether/why they fit this
    /// device. `mode`: automatic | quality | speed. `persona` selects the
    /// needed capability (e.g. code models for software_engineer); "" = general.
    pub fn models(
        &self,
        hints_json: String,
        mode: String,
        language: String,
        persona: String,
    ) -> Result<String, FfiError> {
        let persona = ara_tools::personas::find(if persona.is_empty() {
            "general"
        } else {
            &persona
        })?;
        let capability: Capability = parse("capability", &format!("\"{}\"", persona.capability))?;
        let hints: HostHints = parse("hints", &hints_json)?;
        let device = ara_device_profiler::collect(self.store.root(), hints)?;
        let installed: BTreeSet<String> = self.store.installed_ids()?.into_iter().collect();
        let mode = match mode.as_str() {
            "quality" => SelectionMode::Quality,
            "speed" => SelectionMode::Speed,
            _ => SelectionMode::Automatic,
        };
        let req = SelectionRequest {
            failed_verification: self.failed_verification(),
            mode,
            capability,
            language: (!language.is_empty() && persona.needs_user_language).then_some(language),
            context: self.context(),
            installed,
        };
        let r = ModelSelector::default().select(
            self.registry.iter(),
            &device,
            &req,
            &self.benchmarks(),
        );
        let loaded = lock(&self.model).as_ref().map(|l| l.id.clone());
        let mut rows: Vec<ModelRow> = Vec::new();
        for (rank, c) in r.candidates.iter().enumerate() {
            let spec = self.spec(&c.model_id)?;
            rows.push(ModelRow {
                install: self.store.state(spec),
                loaded: loaded.as_deref() == Some(spec.id.as_str()),
                candidate: Some(c),
                excluded: None,
                recommended: rank == 0,
                spec,
            });
        }
        for (id, why) in &r.excluded {
            let spec = self.spec(id)?;
            rows.push(ModelRow {
                install: self.store.state(spec),
                loaded: false,
                candidate: None,
                excluded: Some(why),
                recommended: false,
                spec,
            });
        }
        to_json(&serde_json::json!({
            "device": device,
            "effective_mode": r.effective_mode,
            "models": rows,
            "storage": self.storage_info_value()?,
        }))
    }

    // ------------------------------------------------------------ models

    pub fn download_model(
        &self,
        id: String,
        listener: Box<dyn ProgressListener>,
        cancel: Arc<CancelHandle>,
    ) -> Result<(), FfiError> {
        let spec = self.spec(&id)?;
        self.store
            .download(spec, &self.policy(), &cancel.0, &mut |p| {
                listener.on_progress(p.downloaded, p.total)
            })?;
        Ok(())
    }

    pub fn delete_model(&self, id: String) -> Result<(), FfiError> {
        let in_use = lock(&self.model).as_ref().is_some_and(|l| l.id == id);
        self.store.delete(&id, in_use)?;
        Ok(())
    }

    /// Loads (or switches to) a model. Only one model is resident at a time.
    pub fn load_model(&self, id: String) -> Result<(), FfiError> {
        let spec = self.spec(&id)?;
        if self.store.state(spec) != InstallState::Installed {
            return Err(AraError::NotFound("model is not downloaded".into()).into());
        }
        let mut slot = lock(&self.model);
        if slot.as_ref().is_some_and(|l| l.id == id) {
            return Ok(());
        }
        *slot = None; // free the previous model's memory first
        let model = LlamaEngine.load(
            &self.store.model_path(&id),
            &LoadParams {
                context: self.context().min(spec.max_context),
                ..Default::default()
            },
        )?;
        *slot = Some(Loaded { id, model });
        Ok(())
    }

    /// Frees model memory (call when the app goes to background / low memory).
    pub fn unload_model(&self) {
        *lock(&self.model) = None;
    }

    pub fn loaded_model(&self) -> Option<String> {
        lock(&self.model).as_ref().map(|l| l.id.clone())
    }

    /// Runs the capability test suite on the loaded model (takes a minute or
    /// two) and stores the result; recommendations then use it.
    pub fn verify_loaded(&self) -> Result<String, FfiError> {
        let mut slot = lock(&self.model);
        let l = slot
            .as_mut()
            .ok_or_else(|| AraError::NotFound("no model loaded".into()))?;
        let r = ara_coordinator::verify::verify_model(&mut *l.model, &l.id)?;
        let mut all = self.verifications();
        all.retain(|x| x.model_id != r.model_id);
        all.push(r.clone());
        write_json(&self.home.join("verification.json"), &all)?;
        to_json(&r)
    }

    pub fn verification_reports(&self) -> Result<String, FfiError> {
        to_json(&self.verifications())
    }

    /// Short benchmark of the loaded model; stored to improve recommendations.
    pub fn benchmark_loaded(&self) -> Result<String, FfiError> {
        let base = ara_device_profiler::process_memory()?;
        let mut slot = lock(&self.model);
        let l = slot
            .as_mut()
            .ok_or_else(|| AraError::NotFound("no model loaded".into()))?;
        let stats = l.model.generate_chat(
            &[ara_inference::ChatMessage::new(
                ara_inference::Role::User,
                "Write one short paragraph about the Nile.",
            )],
            &GenerationParams {
                max_tokens: 48,
                temperature: 0.0,
                ..Default::default()
            },
            &CancellationToken::new(),
            &mut |_| {},
        )?;
        let mem = ara_device_profiler::process_memory()?;
        let spec = self.spec(&l.id)?;
        let b = Benchmark {
            model_id: l.id.clone(),
            context: l.model.context_len(),
            generation_tokens_per_sec: stats.tokens_per_sec,
            time_to_first_token_ms: stats.time_to_first_token_ms as u32,
            load_time_ms: 0,
            // RSS includes mmapped weights touched so far; never below file size.
            peak_memory_bytes: mem
                .peak_rss_bytes
                .saturating_sub(base.rss_bytes)
                .max(spec.file_size_bytes),
            stable: true,
        };
        let mut all = self.benchmarks();
        all.retain(|x| !(x.model_id == b.model_id && x.context == b.context));
        all.push(b.clone());
        write_json(&self.home.join("benchmarks.json"), &all)?;
        to_json(&b)
    }

    // ------------------------------------------------------------ storage

    pub fn storage_info(&self) -> Result<String, FfiError> {
        to_json(&self.storage_info_value()?)
    }

    /// `max_bytes = 0` removes the cap.
    pub fn set_storage_cap(&self, max_bytes: u64) -> Result<(), FfiError> {
        let mut s = lock(&self.settings);
        s.max_model_storage_bytes = (max_bytes > 0).then_some(max_bytes);
        write_json(&self.home.join("settings.json"), &*s)
    }

    /// Context window for the next load (512..=32768). Smaller saves RAM.
    pub fn set_context_tokens(&self, tokens: u32) -> Result<(), FfiError> {
        if !(512..=32768).contains(&tokens) {
            return Err(AraError::InvalidInput("context must be 512..=32768".into()).into());
        }
        let mut s = lock(&self.settings);
        s.context_tokens = Some(tokens);
        write_json(&self.home.join("settings.json"), &*s)
    }

    // ------------------------------------------------------------ conversations

    pub fn conversations(&self) -> Result<String, FfiError> {
        to_json(&self.conversations.list()?)
    }

    /// `persona`: specialist mode id from `personas()` ("" = general).
    pub fn create_conversation(&self, title: String, persona: String) -> Result<String, FfiError> {
        let persona = if persona.is_empty() {
            "general".to_string()
        } else {
            persona
        };
        ara_tools::personas::find(&persona)?;
        Ok(self.conversations.create(&title, &persona)?.id)
    }

    pub fn personas(&self) -> Result<String, FfiError> {
        to_json(&ara_tools::personas::builtin_personas())
    }

    pub fn conversation(&self, id: String) -> Result<String, FfiError> {
        to_json(&self.conversations.get(&id)?)
    }

    pub fn rename_conversation(&self, id: String, title: String) -> Result<(), FfiError> {
        Ok(self.conversations.rename(&id, &title)?)
    }

    pub fn delete_conversation(&self, id: String) -> Result<(), FfiError> {
        Ok(self.conversations.delete(&id)?)
    }

    /// Runs one turn on the loaded model, streaming visible text. Saves the
    /// turn. Returns a `TurnResult` JSON (may contain a pending confirmation).
    pub fn send_message(
        &self,
        conversation_id: String,
        text: String,
        listener: Box<dyn TextListener>,
        cancel: Arc<CancelHandle>,
    ) -> Result<String, FfiError> {
        let conv = self.conversations.get(&conversation_id)?;
        let grants = lock(&self.grants).clone();
        let mut slot = lock(&self.model);
        let l = slot
            .as_mut()
            .ok_or_else(|| AraError::NotFound("no model loaded".into()))?;
        let persona = ara_tools::personas::find(&conv.persona)
            .unwrap_or_else(|_| ara_tools::personas::find("general").expect("built-in"));
        let memory = lock(&self.index).memory_prompt(1200)?;
        let services = IndexServices(&self.index);
        let c = Coordinator {
            tools: &self.tools,
            skills: &self.skills,
            grants: &grants,
            services: Some(&services),
            extra_system: &memory,
            persona: Some(&persona),
        };
        let params = GenerationParams {
            max_tokens: 512,
            ..Default::default()
        };
        let r = c.run_turn(
            &mut *l.model,
            &conv.messages,
            &text,
            &params,
            &cancel.0,
            &mut |t| listener.on_text(t.to_string()),
        )?;
        self.conversations
            .append(&conversation_id, &r.new_messages)?;
        if conv.messages.is_empty() {
            let title: String = text.chars().take(40).collect();
            let _ = self.conversations.rename(&conversation_id, &title);
        }
        let mut v = serde_json::to_value(&r).map_err(|e| AraError::Internal(e.to_string()))?;
        v["model_id"] = serde_json::Value::String(l.id.clone());
        to_json(&v)
    }

    /// The user's answer to a pending confirmation. Returns a `Dispatch`
    /// JSON: `run_on_host` → the app executes the action now.
    pub fn confirm_action(&self, call_json: String, approved: bool) -> Result<String, FfiError> {
        let call: ToolCall = parse("call", &call_json)?;
        if !approved {
            return to_json(&Dispatch::Denied {
                tool: call.tool,
                reason: "declined by user".into(),
            });
        }
        let grants = lock(&self.grants).clone();
        match self.tools.dispatch(&call, &grants, true) {
            // Service tools (e.g. remember) run right here once approved.
            Dispatch::RunService { call } => {
                let output = IndexServices(&self.index).run(&call)?;
                to_json(&Dispatch::Done {
                    tool: call.tool,
                    output,
                })
            }
            other => to_json(&other),
        }
    }

    // ------------------------------------------------------------ missions

    /// Starts a mission. `max_steps` 1..=2000, `max_minutes` 1..=1440 (24 h).
    pub fn create_mission(
        &self,
        goal: String,
        persona: String,
        max_steps: u32,
        max_minutes: u32,
    ) -> Result<String, FfiError> {
        let persona = if persona.is_empty() {
            "general".to_string()
        } else {
            persona
        };
        ara_tools::personas::find(&persona)?;
        let budget = ara_missions::Budget::new(max_steps, max_minutes as u64 * 60_000)?;
        to_json(&self.missions.create(&goal, &persona, budget, now_ms())?)
    }

    pub fn missions(&self) -> Result<String, FfiError> {
        to_json(&self.missions.list()?)
    }

    pub fn mission(&self, id: String) -> Result<String, FfiError> {
        to_json(&self.missions.get(&id)?)
    }

    pub fn mission_audit(&self, id: String) -> Result<String, FfiError> {
        to_json(&self.missions.audit_log(&id)?)
    }

    /// `status`: running | paused | cancelled.
    pub fn set_mission_status(&self, id: String, status: String) -> Result<(), FfiError> {
        let st: ara_missions::Status = parse("status", &format!("\"{status}\""))?;
        self.missions.set_status(&id, st, now_ms())?;
        Ok(())
    }

    /// Ids of missions that should keep running (after an app restart).
    pub fn resumable_missions(&self) -> Result<Vec<String>, FfiError> {
        Ok(self.missions.resumable()?)
    }

    /// Runs one step of a mission on the loaded model (call from a
    /// background worker). Returns the updated mission JSON.
    pub fn run_mission_step(
        &self,
        id: String,
        cancel: Arc<CancelHandle>,
    ) -> Result<String, FfiError> {
        let m0 = self.missions.get(&id)?;
        let persona = ara_tools::personas::find(&m0.persona)?;
        let grants = lock(&self.grants).clone();
        let memory = lock(&self.index).memory_prompt(1200)?;
        let services = ara_coordinator::IndexServices(&self.index);
        let mut slot = lock(&self.model);
        let loaded = slot
            .as_mut()
            .ok_or_else(|| AraError::NotFound("no model loaded".into()))?;
        let mut err: Option<AraError> = None;
        let m = self.missions.step(&id, now_ms(), &mut |m| {
            let c = Coordinator {
                tools: &self.tools,
                skills: &self.skills,
                grants: &grants,
                services: Some(&services),
                extra_system: &memory,
                persona: Some(&persona),
            };
            let recent: Vec<String> = m.recent.iter().rev().take(5).rev().map(|r| format!("- {}", r.note)).collect();
            let prompt = format!(
                "Mission goal: {}\nStep {} of at most {}.\nProgress so far:\n{}\n\nDo the next useful step toward the goal. \
When the goal is fully achieved, start your reply with DONE: followed by the result. Otherwise end with one line summarizing what you did.",
                m.goal,
                m.steps_done,
                m.budget.max_steps,
                if recent.is_empty() { "(nothing yet)".to_string() } else { recent.join("\n") }
            );
            let params = GenerationParams { max_tokens: 400, ..Default::default() };
            match c.run_turn(&mut *loaded.model, &[], &prompt, &params, &cancel.0, &mut |_| {}) {
                Ok(r) => {
                    if let Some(p) = r.pending_confirmation {
                        return ara_missions::StepOutcome::NeedsApproval {
                            summary: p.summary,
                            call: serde_json::to_value(&p.call).unwrap_or_default(),
                        };
                    }
                    let text = r.text.trim().to_string();
                    match text.strip_prefix("DONE:") {
                        Some(res) => ara_missions::StepOutcome::Done { result: res.trim().to_string() },
                        None => ara_missions::StepOutcome::Progress {
                            note: text.lines().last().unwrap_or("working").to_string(),
                        },
                    }
                }
                Err(e) => {
                    let reason = e.to_string();
                    err = Some(e);
                    ara_missions::StepOutcome::Failed { reason }
                }
            }
        })?;
        if let Some(AraError::Cancelled) = err {
            // User stopped generation: keep the mission alive and paused.
            self.missions
                .set_status(&id, ara_missions::Status::Paused, now_ms())?;
        }
        to_json(&m)
    }

    /// Resolves an approval from the mission inbox. When approved, returns
    /// the tool call (JSON) for the app to execute through `confirm_action`.
    pub fn decide_mission_approval(
        &self,
        id: String,
        approval_id: u32,
        approve: bool,
    ) -> Result<String, FfiError> {
        match self.missions.decide(&id, approval_id, approve, now_ms())? {
            Some(call) => to_json(&call),
            None => Ok("null".to_string()),
        }
    }

    // ------------------------------------------------------------ security check

    /// Runs the Sentinel checks. `facts_json` = HostSecurityFacts from Android.
    /// The first run records the integrity baseline of the models directory.
    pub fn security_check(&self, facts_json: String) -> Result<String, FfiError> {
        let facts: ara_sentinel::HostSecurityFacts = parse("facts", &facts_json)?;
        let mut findings = ara_sentinel::posture_findings(&facts);
        let mut checks = vec!["device_posture".to_string()];
        let base_path = self.home.join("integrity.json");
        let now = ara_sentinel::snapshot(self.store.root())?;
        match std::fs::read(&base_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<ara_sentinel::Baseline>(&b).ok())
        {
            Some(base) => {
                findings.extend(ara_sentinel::integrity_findings(&base, &now));
                checks.push("file_integrity".into());
            }
            None => checks.push("file_integrity_baseline_created".into()),
        }
        // Remember the current state; changes are re-evaluated next time only
        // for files the user did not add/remove through ARA itself.
        write_json(&base_path, &now)?;
        let tcp = std::fs::read_to_string("/proc/net/tcp").unwrap_or_default()
            + &std::fs::read_to_string("/proc/net/tcp6").unwrap_or_default();
        if !tcp.is_empty() {
            findings.extend(ara_sentinel::network_findings(
                &ara_sentinel::listening_ports(&tcp),
            ));
            checks.push("listening_ports".into());
        }
        to_json(&ara_sentinel::report(findings, checks))
    }

    // ------------------------------------------------------------ API Hub

    pub fn cloud_presets(&self) -> Result<String, FfiError> {
        to_json(&ara_cloud::PRESETS)
    }

    pub fn providers(&self) -> Result<String, FfiError> {
        to_json(&*lock(&self.providers))
    }

    /// Adds or updates a provider (`Provider` JSON, without key). A `consent`
    /// field in the JSON is ignored; changing the URL revokes consent.
    pub fn save_provider(&self, provider_json: String) -> Result<(), FfiError> {
        let mut p: Provider = parse("provider", &provider_json)?;
        p.base_url = ara_cloud::validate_base_url(&p.base_url)?;
        if p.id.is_empty()
            || !p
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AraError::InvalidInput("bad provider id".into()).into());
        }
        let mut all = lock(&self.providers);
        // Consent is only granted through set_cloud_consent (the consent
        // screen); it survives edits only while the endpoint is unchanged.
        p.consent = false;
        if let Some(old) = all.iter_mut().find(|x| x.id == p.id) {
            p.consent = old.consent && old.base_url == p.base_url;
            *old = p;
        } else {
            all.push(p);
        }
        write_json(&self.home.join("providers.json"), &*all)
    }

    pub fn delete_provider(&self, id: String) -> Result<(), FfiError> {
        let mut all = lock(&self.providers);
        all.retain(|p| p.id != id);
        write_json(&self.home.join("providers.json"), &*all)?;
        drop(all);
        let mut slot = lock(&self.model);
        if slot
            .as_ref()
            .is_some_and(|l| l.id.starts_with(&format!("cloud:{id}/")))
        {
            *slot = None;
        }
        Ok(())
    }

    pub fn set_cloud_consent(&self, id: String, consent: bool) -> Result<(), FfiError> {
        let mut all = lock(&self.providers);
        let p = all
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| AraError::NotFound("provider".into()))?;
        p.consent = consent;
        write_json(&self.home.join("providers.json"), &*all)
    }

    /// "Test connection": lists the provider's models with this key.
    pub fn cloud_models(&self, base_url: String, api_key: String) -> Result<Vec<String>, FfiError> {
        // TODO(T1.7): thread a real Strict Offline Mode setting through here
        // once it has an Android toggle; the enforcement primitive itself
        // (`NetworkPolicy::StrictOffline`) is implemented and tested in
        // ara-cloud today.
        Ok(ara_cloud::list_models(&base_url, &api_key, ara_cloud::NetworkPolicy::Online)?)
    }

    /// Makes a configured provider the active model. Requires consent. The
    /// key lives only in memory until `unload_model`.
    pub fn use_cloud_model(&self, provider_id: String, api_key: String) -> Result<(), FfiError> {
        let p = lock(&self.providers)
            .iter()
            .find(|p| p.id == provider_id)
            .cloned()
            .ok_or_else(|| AraError::NotFound("provider".into()))?;
        if !p.consent {
            return Err(AraError::InvalidInput(
                "consent required before sending data to this provider".into(),
            )
            .into());
        }
        let m = ara_cloud::CloudModel::new(
            &p.base_url,
            &api_key,
            &p.model,
            32_768,
            p.shield,
            ara_cloud::NetworkPolicy::Online, // TODO(T1.7): see cloud_models above
        )?;
        *lock(&self.model) = Some(Loaded {
            id: format!("cloud:{}/{}", p.id, p.model),
            model: Box::new(m),
        });
        Ok(())
    }

    // ------------------------------------------------------------ documents & memory

    /// Imports a file the user picked (bytes read by the app via SAF).
    pub fn import_document(&self, name: String, bytes: Vec<u8>) -> Result<String, FfiError> {
        to_json(&lock(&self.index).add_document(&name, &bytes)?)
    }

    pub fn documents(&self) -> Result<String, FfiError> {
        to_json(&lock(&self.index).documents()?)
    }

    pub fn delete_document(&self, id: i64) -> Result<(), FfiError> {
        Ok(lock(&self.index).delete_document(id)?)
    }

    pub fn search_documents(&self, query: String) -> Result<String, FfiError> {
        to_json(&lock(&self.index).search(&query, 20)?)
    }

    pub fn memories(&self) -> Result<String, FfiError> {
        to_json(&lock(&self.index).memories()?)
    }

    pub fn add_memory(&self, text: String) -> Result<(), FfiError> {
        lock(&self.index).add_memory(&text)?;
        Ok(())
    }

    pub fn set_memory_enabled(&self, id: i64, enabled: bool) -> Result<(), FfiError> {
        Ok(lock(&self.index).update_memory(id, None, Some(enabled))?)
    }

    pub fn delete_memory(&self, id: i64) -> Result<(), FfiError> {
        Ok(lock(&self.index).delete_memory(id)?)
    }

    // ------------------------------------------------------------ skills & permissions

    pub fn skills(&self) -> Result<String, FfiError> {
        let rows: Vec<_> = self
            .skills
            .iter()
            .map(|s| {
                let mut perms: Vec<Permission> = s
                    .tools
                    .iter()
                    .filter_map(|t| self.tools.get(t))
                    .flat_map(|m| m.permissions.iter().copied())
                    .collect();
                perms.sort();
                perms.dedup();
                serde_json::json!({ "skill": s, "permissions": perms })
            })
            .collect();
        to_json(&rows)
    }

    pub fn permissions(&self) -> Result<String, FfiError> {
        to_json(&*lock(&self.grants))
    }

    /// `permission`: snake_case name; `grant`: denied | ask_every_time | always.
    pub fn set_permission(&self, permission: String, grant: String) -> Result<(), FfiError> {
        let p: Permission = parse("permission", &format!("\"{permission}\""))?;
        let g: Grant = parse("grant", &format!("\"{grant}\""))?;
        let mut gs = lock(&self.grants);
        gs.set(p, g);
        write_json(&self.home.join("permissions.json"), &*gs)
    }
}

impl AraCore {
    fn storage_info_value(&self) -> Result<serde_json::Value, FfiError> {
        let s = lock(&self.settings).clone();
        Ok(serde_json::json!({
            "models_bytes": self.store.used_bytes()?,
            "cap_bytes": s.max_model_storage_bytes,
            "free_bytes": ara_device_profiler::free_storage_bytes(self.store.root())?,
            "context_tokens": self.context(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Collect(Mutex<String>);
    impl TextListener for Arc<Collect> {
        fn on_text(&self, t: String) {
            self.0.lock().unwrap().push_str(&t)
        }
    }

    #[test]
    fn models_lists_whole_catalog_with_fit_info() {
        let d = tempfile::tempdir().unwrap();
        let core = AraCore::new(d.path().to_string_lossy().into()).unwrap();
        let v: serde_json::Value = serde_json::from_str(
            &core
                .models("{}".into(), "automatic".into(), "ar".into(), String::new())
                .unwrap(),
        )
        .unwrap();
        let rows = v["models"].as_array().unwrap();
        assert_eq!(rows.len(), core.registry.len());
        assert!(rows
            .iter()
            .all(|r| r["candidate"].is_null() != r["excluded"].is_null()));
        assert_eq!(
            rows.iter()
                .filter(|r| r["recommended"] == true)
                .count()
                .min(1),
            1.min(rows.len())
        );
        assert!(v["storage"]["free_bytes"].as_u64().unwrap() > 0);
    }

    #[test]
    fn personas_drive_model_recommendations() {
        let d = tempfile::tempdir().unwrap();
        let core = AraCore::new(d.path().to_string_lossy().into()).unwrap();
        let v: serde_json::Value = serde_json::from_str(
            &core
                .models(
                    "{}".into(),
                    "automatic".into(),
                    "ar".into(),
                    "software_engineer".into(),
                )
                .unwrap(),
        )
        .unwrap();
        let ok: Vec<String> = v["models"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| !r["candidate"].is_null())
            .map(|r| r["id"].as_str().unwrap().to_string())
            .collect();
        // Code mode: coder models qualify even though they don't list Arabic.
        assert!(ok.iter().any(|id| id.contains("coder")), "{ok:?}");
        assert!(
            !ok.iter().any(|id| id.starts_with("qwen2.5-0.5b")),
            "no code capability"
        );
        assert!(core
            .create_conversation("x".into(), "hacker".into())
            .is_err());
        let id = core
            .create_conversation("x".into(), "security".into())
            .unwrap();
        assert!(core
            .conversation(id)
            .unwrap()
            .contains("\"persona\":\"security\""));
        assert!(core.personas().unwrap().contains("software_engineer"));
    }

    #[test]
    fn missions_and_security_through_core() {
        let d = tempfile::tempdir().unwrap();
        let core = AraCore::new(d.path().to_string_lossy().into()).unwrap();
        assert!(
            core.create_mission("x".into(), "general".into(), 5, 1441)
                .is_err(),
            "max 24h"
        );
        let m: serde_json::Value = serde_json::from_str(
            &core
                .create_mission("راجع الملفات".into(), "analyst".into(), 10, 60)
                .unwrap(),
        )
        .unwrap();
        let id = m["id"].as_str().unwrap().to_string();
        assert_eq!(core.resumable_missions().unwrap(), vec![id.clone()]);
        let err = core
            .run_mission_step(id.clone(), CancelHandle::new())
            .unwrap_err();
        assert!(err.to_string().contains("no model"));
        core.set_mission_status(id.clone(), "paused".into())
            .unwrap();
        assert!(core
            .set_mission_status(id.clone(), "completed".into())
            .is_err());
        assert!(core.mission_audit(id).unwrap().contains("user_set_status"));

        // Security: first run creates a baseline, tampering is then reported.
        let facts = r#"{"screen_lock_set":false}"#;
        let r1: serde_json::Value =
            serde_json::from_str(&core.security_check(facts.into()).unwrap()).unwrap();
        assert_eq!(r1["findings"][0]["id"], "no_screen_lock");
        std::fs::write(d.path().join("models/x.gguf"), b"1").unwrap();
        core.security_check("{}".into()).unwrap(); // records x.gguf
        std::fs::write(d.path().join("models/x.gguf"), b"2").unwrap();
        let r3: serde_json::Value =
            serde_json::from_str(&core.security_check("{}".into()).unwrap()).unwrap();
        assert!(r3["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["id"] == "file_modified"));
    }

    #[test]
    fn api_hub_requires_consent_and_never_stores_keys() {
        let d = tempfile::tempdir().unwrap();
        let core = AraCore::new(d.path().to_string_lossy().into()).unwrap();
        assert!(core.cloud_presets().unwrap().contains("api.moonshot.ai"));
        let p = r#"{"id":"kimi","name":"Kimi","base_url":"https://api.moonshot.ai/v1/","model":"kimi-k3"}"#;
        core.save_provider(p.replace("}", ",\"consent\":true}"))
            .unwrap();
        assert!(
            core.providers().unwrap().contains("\"consent\":false"),
            "JSON can't self-grant consent"
        );
        assert!(core
            .save_provider(r#"{"id":"x","name":"x","base_url":"http://evil","model":"m"}"#.into())
            .is_err());
        let err = core
            .use_cloud_model("kimi".into(), "sk-secret-value-1234".into())
            .unwrap_err();
        assert!(err.to_string().contains("consent"));
        core.set_cloud_consent("kimi".into(), true).unwrap();
        core.use_cloud_model("kimi".into(), "sk-secret-value-1234".into())
            .unwrap();
        assert_eq!(core.loaded_model().unwrap(), "cloud:kimi/kimi-k3");
        let saved = std::fs::read_to_string(d.path().join("providers.json")).unwrap();
        assert!(!saved.contains("sk-secret"), "key never written to disk");
        assert!(saved.contains("\"shield\":true"));
        // Changing the endpoint revokes consent.
        core.save_provider(r#"{"id":"kimi","name":"Kimi","base_url":"https://other.example/v1","model":"kimi-k3","consent":true}"#.into()).unwrap();
        assert!(core.providers().unwrap().contains("\"consent\":false"));
        core.delete_provider("kimi".into()).unwrap();
        assert!(
            core.loaded_model().is_none(),
            "deleting the provider drops the session key"
        );
    }

    #[test]
    fn settings_and_permissions_persist() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().to_string_lossy().to_string();
        {
            let core = AraCore::new(p.clone()).unwrap();
            core.set_permission("compose_messages".into(), "always".into())
                .unwrap();
            core.set_storage_cap(5 << 30).unwrap();
            core.set_context_tokens(2048).unwrap();
            assert!(core
                .set_permission("root_access".into(), "always".into())
                .is_err());
            assert!(core.set_context_tokens(10).is_err());
        }
        let core = AraCore::new(p).unwrap();
        assert!(core.permissions().unwrap().contains("compose_messages"));
        let s: serde_json::Value = serde_json::from_str(&core.storage_info().unwrap()).unwrap();
        assert_eq!(s["cap_bytes"], 5u64 << 30);
        assert_eq!(s["context_tokens"], 2048);
    }

    #[test]
    fn actions_require_model_and_confirmation_rules_hold() {
        let d = tempfile::tempdir().unwrap();
        let core = AraCore::new(d.path().to_string_lossy().into()).unwrap();
        let conv = core.create_conversation("t".into(), String::new()).unwrap();
        let err = core
            .send_message(
                conv,
                "hi".into(),
                Box::new(Arc::new(Collect(Mutex::new(String::new())))),
                CancelHandle::new(),
            )
            .unwrap_err();
        assert!(
            matches!(err, FfiError::InvalidInput { .. }),
            "no model loaded"
        );
        assert!(
            core.load_model("qwen3-0.6b-q8_0".into()).is_err(),
            "not downloaded"
        );

        let call = r#"{"tool":"flashlight","arguments":{"on":true}}"#;
        let d1: serde_json::Value =
            serde_json::from_str(&core.confirm_action(call.into(), false).unwrap()).unwrap();
        assert_eq!(d1["outcome"], "denied");
        // Approved but permission not granted -> still denied by the core.
        let d2: serde_json::Value =
            serde_json::from_str(&core.confirm_action(call.into(), true).unwrap()).unwrap();
        assert_eq!(d2["outcome"], "denied");
        core.set_permission("device_controls".into(), "ask_every_time".into())
            .unwrap();
        let d3: serde_json::Value =
            serde_json::from_str(&core.confirm_action(call.into(), true).unwrap()).unwrap();
        assert_eq!(d3["outcome"], "run_on_host");
    }

    #[test]
    fn documents_and_memory_through_core_and_tools() {
        let d = tempfile::tempdir().unwrap();
        let core = AraCore::new(d.path().to_string_lossy().into()).unwrap();
        core.import_document(
            "خطة.md".into(),
            "# الخطة\nالميزانية ٥٠٠٠ جنيه".as_bytes().to_vec(),
        )
        .unwrap();
        assert!(core.documents().unwrap().contains("خطة.md"));
        let hits: serde_json::Value =
            serde_json::from_str(&core.search_documents("الميزانيه".into()).unwrap()).unwrap();
        assert_eq!(hits[0]["document"], "خطة.md");
        assert!(core.import_document("x.exe".into(), vec![0, 1]).is_err());

        // Service tool via the agent path: needs permission, then runs locally.
        let search = r#"{"tool":"search_documents","arguments":{"query":"5000"}}"#;
        let denied: serde_json::Value =
            serde_json::from_str(&core.confirm_action(search.into(), true).unwrap()).unwrap();
        assert_eq!(denied["outcome"], "denied");
        core.set_permission("read_user_files".into(), "always".into())
            .unwrap();
        let ok: serde_json::Value =
            serde_json::from_str(&core.confirm_action(search.into(), true).unwrap()).unwrap();
        assert_eq!(ok["output"]["results"][0]["source"], "خطة.md:2");

        core.set_permission("memory".into(), "ask_every_time".into())
            .unwrap();
        let rem = r#"{"tool":"remember","arguments":{"fact":"بحب الشاي"}}"#;
        core.confirm_action(rem.into(), true).unwrap();
        let mems: serde_json::Value = serde_json::from_str(&core.memories().unwrap()).unwrap();
        assert_eq!(mems[0]["text"], "بحب الشاي");
        let id = mems[0]["id"].as_i64().unwrap();
        core.set_memory_enabled(id, false).unwrap();
        core.delete_memory(id).unwrap();
        assert_eq!(core.memories().unwrap(), "[]");
    }

    #[test]
    fn real_model_end_to_end_when_available() {
        // Uses ARA_TEST_HOME (a data dir with an installed model) if set.
        let Some(home) = std::env::var_os("ARA_TEST_HOME") else {
            eprintln!("skipped: ARA_TEST_HOME not set");
            return;
        };
        let core = AraCore::new(home.to_string_lossy().into()).unwrap();
        let id = core.store.installed_ids().unwrap().remove(0);
        core.load_model(id.clone()).unwrap();
        assert_eq!(core.loaded_model(), Some(id.clone()));
        assert!(core.delete_model(id).is_err(), "cannot delete loaded model");
        let conv = core
            .create_conversation(String::new(), "general".into())
            .unwrap();
        let sink = Arc::new(Collect(Mutex::new(String::new())));
        let r = core
            .send_message(
                conv.clone(),
                "احسب 12*12".into(),
                Box::new(sink.clone()),
                CancelHandle::new(),
            )
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["routed_skills"][0], "math");
        assert!(!sink.0.lock().unwrap().contains("<tool_call>"));
        let saved: serde_json::Value =
            serde_json::from_str(&core.conversation(conv).unwrap()).unwrap();
        assert!(saved["messages"].as_array().unwrap().len() >= 2);
        assert_eq!(saved["title"], "احسب 12*12");
        core.unload_model();
        assert!(core.loaded_model().is_none());
    }
}
