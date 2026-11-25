//! Model selection: exclude incompatible models first, then rank the rest.
//!
//! Decisions use *available* RAM and real storage, never a fixed
//! "RAM class -> model" table. When a model has no measured benchmark on this
//! device, its ranking is flagged `estimated = true` so the UI can say so.

use ara_device_profiler::{DeviceProfile, ThermalState};
use ara_model_registry::{Capability, ModelSpec};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    #[default]
    Automatic,
    Quality,
    Speed,
}

/// A measurement taken on *this* device by the inference module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Benchmark {
    pub model_id: String,
    pub context: u32,
    pub generation_tokens_per_sec: f32,
    pub time_to_first_token_ms: u32,
    pub load_time_ms: u32,
    pub peak_memory_bytes: u64,
    /// False if the run crashed, was OOM-killed, or produced errors.
    pub stable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionRequest {
    pub mode: SelectionMode,
    pub capability: Capability,
    /// BCP-47 tag of the conversation language, if known.
    pub language: Option<String>,
    pub context: u32,
    /// Ids already on disk (they don't need free storage).
    #[serde(default)]
    pub installed: BTreeSet<String>,
    /// Capabilities that were *tested on this device* and failed
    /// (model id -> capabilities). Overrides catalog metadata.
    #[serde(default)]
    pub failed_verification: std::collections::BTreeMap<String, BTreeSet<Capability>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Exclusion {
    MissingCapability,
    UnsupportedLanguage,
    ContextTooLarge {
        max: u32,
    },
    InsufficientMemory {
        required: u64,
        budget: u64,
    },
    InsufficientStorage {
        required: u64,
        free: u64,
    },
    UnstableOnDevice,
    /// The catalog lists the capability but our own test of it failed.
    FailedVerification,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub model_id: String,
    /// 0..=1, higher is better. Only comparable within one result.
    pub score: f32,
    /// True when no benchmark on this device backs the ranking.
    pub estimated: bool,
    /// Measured decode speed, or a bandwidth-based estimate when unmeasured.
    pub estimated_tokens_per_sec: f32,
    pub required_memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SelectionResult {
    /// Best first.
    pub candidates: Vec<Candidate>,
    pub excluded: Vec<(String, Exclusion)>,
    /// The mode actually applied (Automatic may fall back to Speed when hot).
    pub effective_mode: Option<SelectionMode>,
}

/// Tunables. Defaults are conservative; they are policy, not measurements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectorPolicy {
    /// Memory kept free for the OS/UI on top of the model's requirement.
    pub ram_headroom_bytes: u64,
    /// Fraction of available RAM a model may use.
    pub ram_budget_fraction: f64,
    /// Storage kept free after a download.
    pub storage_reserve_bytes: u64,
    /// Below this speed a model is penalized in Automatic mode.
    pub min_usable_tokens_per_sec: f32,
    /// Below this speed a model is penalized even in Quality mode.
    pub min_tolerable_tokens_per_sec: f32,
    /// Effective memory bandwidth assumed before any benchmark exists.
    /// Conservative for mid-range phones; replaced by calibration.
    pub assumed_bandwidth_bytes_per_sec: f64,
}

impl Default for SelectorPolicy {
    fn default() -> Self {
        Self {
            ram_headroom_bytes: 512 << 20,
            ram_budget_fraction: 0.85,
            storage_reserve_bytes: 1 << 30,
            min_usable_tokens_per_sec: 8.0,
            min_tolerable_tokens_per_sec: 2.0,
            assumed_bandwidth_bytes_per_sec: 6.0e9,
        }
    }
}

#[derive(Default)]
pub struct ModelSelector {
    pub policy: SelectorPolicy,
}

impl ModelSelector {
    fn memory_budget(&self, device: &DeviceProfile) -> u64 {
        let frac = (device.available_ram_bytes as f64 * self.policy.ram_budget_fraction) as u64;
        frac.saturating_sub(self.policy.ram_headroom_bytes)
    }

    /// Checks a single model (also used for manual selection in the UI).
    pub fn check(
        &self,
        model: &ModelSpec,
        device: &DeviceProfile,
        req: &SelectionRequest,
        bench: Option<&Benchmark>,
    ) -> Result<u64, Exclusion> {
        if !model.capabilities.contains(&req.capability) {
            return Err(Exclusion::MissingCapability);
        }
        if req
            .failed_verification
            .get(&model.id)
            .is_some_and(|f| f.contains(&req.capability))
        {
            return Err(Exclusion::FailedVerification);
        }
        if let Some(lang) = &req.language {
            if !model.supports_language(lang) {
                return Err(Exclusion::UnsupportedLanguage);
            }
        }
        if req.context > model.max_context {
            return Err(Exclusion::ContextTooLarge {
                max: model.max_context,
            });
        }
        if bench.is_some_and(|b| !b.stable) {
            return Err(Exclusion::UnstableOnDevice);
        }
        let estimate = model.estimated_runtime_bytes(req.context);
        // A measurement at >= the requested context beats the estimate.
        let required = match bench {
            Some(b) if b.context >= req.context => b.peak_memory_bytes.max(model.file_size_bytes),
            _ => estimate,
        };
        let budget = self.memory_budget(device);
        if required > budget {
            return Err(Exclusion::InsufficientMemory { required, budget });
        }
        if !req.installed.contains(&model.id) {
            let need = model
                .file_size_bytes
                .saturating_add(self.policy.storage_reserve_bytes);
            if need > device.free_storage_bytes {
                return Err(Exclusion::InsufficientStorage {
                    required: need,
                    free: device.free_storage_bytes,
                });
            }
        }
        Ok(required)
    }

    pub fn select<'a>(
        &self,
        models: impl IntoIterator<Item = &'a ModelSpec>,
        device: &DeviceProfile,
        req: &SelectionRequest,
        benchmarks: &[Benchmark],
    ) -> SelectionResult {
        let bench_by_id: BTreeMap<&str, &Benchmark> = benchmarks
            .iter()
            .map(|b| (b.model_id.as_str(), b))
            .collect();

        let mode = match req.mode {
            SelectionMode::Automatic
                if device
                    .hints
                    .thermal
                    .is_some_and(|t| t >= ThermalState::Severe) =>
            {
                SelectionMode::Speed
            }
            m => m,
        };

        let mut result = SelectionResult {
            effective_mode: Some(mode),
            ..Default::default()
        };
        let mut ok: Vec<(&ModelSpec, u64, Option<&Benchmark>)> = Vec::new();
        for m in models {
            let b = bench_by_id.get(m.id.as_str()).copied();
            match self.check(m, device, req, b) {
                Ok(required) => ok.push((m, required, b)),
                Err(e) => result.excluded.push((m.id.clone(), e)),
            }
        }
        if ok.is_empty() {
            return result;
        }

        // Decode on CPU is memory-bandwidth bound: every token reads roughly
        // the whole weight file. Effective bandwidth is calibrated from this
        // device's benchmarks (median of tok/s x file size); without any, a
        // conservative assumption is used and results stay `estimated`.
        let mut calib: Vec<f64> = ok
            .iter()
            .filter_map(|(m, _, b)| {
                b.map(|b| b.generation_tokens_per_sec as f64 * m.file_size_bytes as f64)
            })
            .filter(|x| *x > 0.0)
            .collect();
        calib.sort_by(f64::total_cmp);
        let bandwidth = calib
            .get(calib.len() / 2)
            .copied()
            .unwrap_or(self.policy.assumed_bandwidth_bytes_per_sec);
        let tps: Vec<f64> = ok
            .iter()
            .map(|(m, _, b)| match b {
                Some(b) => b.generation_tokens_per_sec as f64,
                None => bandwidth / m.file_size_bytes.max(1) as f64,
            })
            .collect();
        // Quality proxy: log parameter count, slightly reduced for 4-bit.
        let quality: Vec<f64> = ok
            .iter()
            .map(|(m, _, _)| {
                let q = m.quantization.to_ascii_uppercase();
                let qf = if q.starts_with("Q8") || q.starts_with("F16") {
                    1.0
                } else if q.starts_with("Q5") || q.starts_with("Q6") {
                    0.985
                } else {
                    0.96
                };
                (m.parameters_b as f64 + 1.0).ln() * qf
            })
            .collect();
        let norm = |v: &[f64]| -> Vec<f64> {
            let (lo, hi) = v
                .iter()
                .fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
            v.iter()
                .map(|x| if hi > lo { (x - lo) / (hi - lo) } else { 1.0 })
                .collect()
        };
        // Log scale: 2 -> 5 tok/s matters far more to a user than 20 -> 25.
        let log_tps: Vec<f64> = tps.iter().map(|t| t.max(0.01).ln()).collect();
        let (q, s) = (norm(&quality), norm(&log_tps));
        let budget = self.memory_budget(device) as f64;
        let p = &self.policy;

        result.candidates = ok
            .iter()
            .enumerate()
            .map(|(i, (m, required, b))| {
                // Tiers make the order explainable: within Automatic, every
                // "usable" model ranks above every "tolerable" one, etc.
                let usable = tps[i] >= p.min_usable_tokens_per_sec as f64
                    && (*required as f64) <= budget * 0.6;
                let tolerable = tps[i] >= p.min_tolerable_tokens_per_sec as f64;
                let (tier, within) = match mode {
                    SelectionMode::Speed => (0.0, 0.85 * s[i] + 0.15 * q[i]),
                    SelectionMode::Quality => {
                        (if tolerable { 1.0 } else { 0.0 }, 0.85 * q[i] + 0.15 * s[i])
                    }
                    SelectionMode::Automatic => (
                        if usable {
                            2.0
                        } else if tolerable {
                            1.0
                        } else {
                            0.0
                        },
                        // Below "usable", speed matters more than size.
                        if usable {
                            0.8 * q[i] + 0.2 * s[i]
                        } else {
                            0.4 * q[i] + 0.6 * s[i]
                        },
                    ),
                };
                let score = (tier + within) / 3.0;
                Candidate {
                    model_id: m.id.clone(),
                    score: score as f32,
                    estimated: b.is_none(),
                    estimated_tokens_per_sec: tps[i] as f32,
                    required_memory_bytes: *required,
                }
            })
            .collect();
        result.candidates.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.required_memory_bytes.cmp(&b.required_memory_bytes))
                .then_with(|| a.model_id.cmp(&b.model_id))
        });
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ara_device_profiler::HostHints;
    use ara_model_registry::fixtures::spec;

    const GIB: u64 = 1 << 30;

    fn device(total_gb: u64, avail_gb: u64, free_gb: u64) -> DeviceProfile {
        DeviceProfile {
            cpu_features: vec![],
            core_max_khz: vec![],
            tier: String::new(),
            arch: "aarch64".into(),
            cpu_cores: 8,
            total_ram_bytes: total_gb * GIB,
            available_ram_bytes: avail_gb * GIB,
            free_storage_bytes: free_gb * GIB,
            hints: HostHints::default(),
        }
    }

    fn req(mode: SelectionMode) -> SelectionRequest {
        SelectionRequest {
            failed_verification: Default::default(),
            mode,
            capability: Capability::Chat,
            language: Some("ar".into()),
            context: 2048,
            installed: BTreeSet::new(),
        }
    }

    fn models() -> Vec<ModelSpec> {
        vec![
            spec("small", 0.5, 400),
            spec("mid", 1.5, 1000),
            spec("large", 3.0, 2000),
            spec("huge", 8.0, 5000),
        ]
    }

    #[test]
    fn uses_available_not_total_ram() {
        // 8 GB phone but only 3 GiB available: large/huge must be excluded.
        let r = ModelSelector::default().select(
            &models(),
            &device(8, 3, 64),
            &req(SelectionMode::Automatic),
            &[],
        );
        let ids: Vec<_> = r.candidates.iter().map(|c| c.model_id.as_str()).collect();
        assert!(ids.contains(&"small") && ids.contains(&"mid"));
        assert!(r
            .excluded
            .iter()
            .any(|(id, e)| id == "large" && matches!(e, Exclusion::InsufficientMemory { .. })));
    }

    #[test]
    fn quality_and_speed_modes_order_differently() {
        let sel = ModelSelector::default();
        let d = device(16, 12, 64);
        let q = sel.select(&models(), &d, &req(SelectionMode::Quality), &[]);
        let s = sel.select(&models(), &d, &req(SelectionMode::Speed), &[]);
        // "huge" (5 GB) would decode at ~1.2 tok/s at the assumed 6 GB/s, so
        // Quality prefers the largest model that is still tolerable.
        assert_eq!(q.candidates[0].model_id, "large");
        assert_eq!(s.candidates[0].model_id, "small");
        assert!(q.candidates.iter().all(|c| c.estimated));
    }

    #[test]
    fn automatic_prefers_best_quality_at_usable_speed() {
        let r = ModelSelector::default().select(
            &models(),
            &device(16, 12, 64),
            &req(SelectionMode::Automatic),
            &[],
        );
        // Uncalibrated (6 GB/s): small ~14 tok/s is the only "usable" one;
        // then tolerable mid/large, favouring speed; "huge" (~1 tok/s) last.
        let ids: Vec<_> = r.candidates.iter().map(|c| c.model_id.as_str()).collect();
        assert_eq!(ids, ["small", "mid", "large", "huge"]);
    }

    #[test]
    fn benchmarks_calibrate_estimates_for_unmeasured_models() {
        let ms = models();
        // Measured: "small" (400 MiB) at 60 tok/s => ~25 GB/s effective.
        let b = [Benchmark {
            model_id: "small".into(),
            context: 4096,
            generation_tokens_per_sec: 60.0,
            time_to_first_token_ms: 100,
            load_time_ms: 500,
            peak_memory_bytes: 700 << 20,
            stable: true,
        }];
        let r = ModelSelector::default().select(
            &ms,
            &device(16, 12, 64),
            &req(SelectionMode::Automatic),
            &b,
        );
        let large = r.candidates.iter().find(|c| c.model_id == "large").unwrap();
        assert!(large.estimated);
        assert!(
            (large.estimated_tokens_per_sec - 12.0).abs() < 0.5,
            "{large:?}"
        );
        // With a fast device, Automatic moves up to a bigger model.
        assert_ne!(r.candidates[0].model_id, "small");
        assert!(
            !r.candidates
                .iter()
                .find(|c| c.model_id == "small")
                .unwrap()
                .estimated
        );
    }

    #[test]
    fn storage_is_checked_only_for_uninstalled_models() {
        let sel = ModelSelector::default();
        let mut d = device(16, 12, 0);
        d.free_storage_bytes = 1536 << 20; // 1.5 GiB free, 1 GiB reserve
        let mut r = req(SelectionMode::Automatic);
        let res = sel.select(&models(), &d, &r, &[]);
        assert!(res
            .excluded
            .iter()
            .any(|(id, e)| id == "mid" && matches!(e, Exclusion::InsufficientStorage { .. })));
        r.installed.insert("mid".into());
        let res = sel.select(&models(), &d, &r, &[]);
        assert!(res.candidates.iter().any(|c| c.model_id == "mid"));
    }

    #[test]
    fn measured_benchmarks_drive_ranking_and_unstable_is_excluded() {
        let sel = ModelSelector::default();
        let d = device(16, 12, 64);
        let ms = vec![
            spec("a", 1.5, 1000),
            spec("b", 1.5, 1000),
            spec("c", 1.5, 1000),
        ];
        let bench = |id: &str, tps: f32, stable: bool| Benchmark {
            model_id: id.into(),
            context: 4096,
            generation_tokens_per_sec: tps,
            time_to_first_token_ms: 500,
            load_time_ms: 2000,
            peak_memory_bytes: 1200 << 20,
            stable,
        };
        let b = [
            bench("a", 5.0, true),
            bench("b", 20.0, true),
            bench("c", 50.0, false),
        ];
        let r = sel.select(&ms, &d, &req(SelectionMode::Speed), &b);
        assert_eq!(r.candidates[0].model_id, "b");
        assert!(!r.candidates[0].estimated);
        assert!(r
            .excluded
            .iter()
            .any(|(id, e)| id == "c" && *e == Exclusion::UnstableOnDevice));
    }

    #[test]
    fn filters_language_capability_and_context() {
        let sel = ModelSelector::default();
        let d = device(16, 12, 64);
        let mut fr_only = spec("fr", 1.0, 500);
        fr_only.languages = ["fr".to_string()].into_iter().collect();
        let mut emb = spec("emb", 0.1, 100);
        emb.capabilities = [Capability::Embedding].into_iter().collect();
        let mut short = spec("short", 1.0, 500);
        short.max_context = 2048;
        short.default_context = 1024;
        let mut r = req(SelectionMode::Automatic);
        r.context = 4096;
        let res = sel.select(&[fr_only, emb, short], &d, &r, &[]);
        assert!(res.candidates.is_empty());
        let reasons: Vec<_> = res.excluded.iter().map(|(_, e)| e.clone()).collect();
        assert!(reasons.contains(&Exclusion::UnsupportedLanguage));
        assert!(reasons.contains(&Exclusion::MissingCapability));
        assert!(reasons.contains(&Exclusion::ContextTooLarge { max: 2048 }));
    }

    #[test]
    fn hot_device_falls_back_to_speed_in_automatic_mode() {
        let mut d = device(16, 12, 64);
        d.hints.thermal = Some(ThermalState::Severe);
        let r = ModelSelector::default().select(&models(), &d, &req(SelectionMode::Automatic), &[]);
        assert_eq!(r.effective_mode, Some(SelectionMode::Speed));
        assert_eq!(r.candidates[0].model_id, "small");
    }

    #[test]
    fn failed_verification_overrides_catalog_claims() {
        let mut r = req(SelectionMode::Automatic);
        r.capability = Capability::ToolUse;
        let mut m = spec("claims-tools", 1.0, 400);
        m.capabilities.insert(Capability::ToolUse);
        r.failed_verification.insert(
            "claims-tools".into(),
            [Capability::ToolUse].into_iter().collect(),
        );
        let res = ModelSelector::default().select(&[m], &device(16, 12, 64), &r, &[]);
        assert!(res.candidates.is_empty());
        assert_eq!(res.excluded[0].1, Exclusion::FailedVerification);
    }

    #[test]
    fn empty_catalog_yields_empty_result() {
        let r = ModelSelector::default().select(
            &[] as &[ModelSpec],
            &device(8, 4, 64),
            &req(SelectionMode::Automatic),
            &[],
        );
        assert!(r.candidates.is_empty() && r.excluded.is_empty());
    }
}
