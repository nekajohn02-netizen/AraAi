//! Model metadata catalog.
//!
//! The registry only describes models; it never downloads or loads them.
//! ARA ships **no built-in entries**: a catalog is loaded from JSON whose
//! source, license and checksum must be verified before it is distributed
//! (see docs/SECURITY_AND_PRIVACY.md). Entries without a checksum are kept but
//! flagged as unverified so the model manager can refuse to install them.

use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    Gguf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Chat,
    Reasoning,
    Code,
    Embedding,
    ToolUse,
}

/// One downloadable model artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    /// Stable identifier, e.g. `"qwen2.5-1.5b-instruct-q4_k_m"`.
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub format: ModelFormat,
    /// Quantization label as published by the source, e.g. `"Q4_K_M"`.
    pub quantization: String,
    /// Parameter count in billions (informational, used for quality ranking).
    pub parameters_b: f32,
    /// Size of the model file on disk, in bytes.
    pub file_size_bytes: u64,
    /// Extra runtime memory needed beyond the weights at `default_context`
    /// (KV cache, scratch buffers). Bytes per context token.
    pub kv_bytes_per_token: u64,
    /// Fixed runtime overhead in bytes (engine buffers etc.).
    #[serde(default)]
    pub runtime_overhead_bytes: u64,
    pub max_context: u32,
    pub default_context: u32,
    /// BCP-47 language tags the model is documented to support.
    pub languages: BTreeSet<String>,
    pub capabilities: BTreeSet<Capability>,
    /// SPDX identifier or license name as stated by the source.
    pub license: String,
    pub source_url: String,
    /// Lower-case hex SHA-256 of the model file. `None` = unverified.
    #[serde(default)]
    pub sha256: Option<String>,
}

impl ModelSpec {
    /// Estimated resident memory to run this model with `context` tokens.
    /// This is an estimate; real numbers come from benchmarks.
    pub fn estimated_runtime_bytes(&self, context: u32) -> u64 {
        self.file_size_bytes
            .saturating_add(self.kv_bytes_per_token.saturating_mul(context as u64))
            .saturating_add(self.runtime_overhead_bytes)
    }

    pub fn is_checksum_verified(&self) -> bool {
        self.sha256.is_some()
    }

    pub fn supports_language(&self, tag: &str) -> bool {
        // Match primary subtag so "ar-EG" matches a model declaring "ar".
        let primary = tag.split('-').next().unwrap_or(tag).to_ascii_lowercase();
        self.languages.iter().any(|l| {
            let lp = l.split('-').next().unwrap_or(l).to_ascii_lowercase();
            lp == primary
        })
    }

    fn validate(&self) -> AraResult<()> {
        let bad = |m: &str| Err(AraError::InvalidInput(format!("model '{}': {m}", self.id)));
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return bad("id must be non-empty [A-Za-z0-9._-]");
        }
        if self.file_size_bytes == 0 {
            return bad("file_size_bytes must be > 0");
        }
        if self.default_context == 0 || self.default_context > self.max_context {
            return bad("default_context must be in 1..=max_context");
        }
        if !(self.parameters_b.is_finite() && self.parameters_b > 0.0) {
            return bad("parameters_b must be a positive number");
        }
        if self.license.trim().is_empty() {
            return bad("license is required");
        }
        if !self.source_url.starts_with("https://") {
            return bad("source_url must use https");
        }
        if self.capabilities.is_empty() {
            return bad("at least one capability is required");
        }
        if let Some(h) = &self.sha256 {
            if h.len() != 64 || !h.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
                return bad("sha256 must be 64 lower-case hex chars");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogFile {
    schema_version: u32,
    models: Vec<ModelSpec>,
}

/// Validated, id-indexed set of models.
#[derive(Debug, Clone, Default)]
pub struct ModelRegistry {
    models: BTreeMap<String, ModelSpec>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_json(json: &str) -> AraResult<Self> {
        let file: CatalogFile = serde_json::from_str(json)
            .map_err(|e| AraError::InvalidInput(format!("catalog json: {e}")))?;
        if file.schema_version != CATALOG_SCHEMA_VERSION {
            return Err(AraError::InvalidInput(format!(
                "unsupported catalog schema_version {} (expected {CATALOG_SCHEMA_VERSION})",
                file.schema_version
            )));
        }
        let mut reg = Self::new();
        for m in file.models {
            reg.insert(m)?;
        }
        Ok(reg)
    }

    pub fn to_json(&self) -> String {
        let file = CatalogFile {
            schema_version: CATALOG_SCHEMA_VERSION,
            models: self.models.values().cloned().collect(),
        };
        serde_json::to_string_pretty(&file).expect("catalog serialization cannot fail")
    }

    /// Validates and inserts. Duplicate ids are rejected.
    pub fn insert(&mut self, spec: ModelSpec) -> AraResult<()> {
        spec.validate()?;
        if self.models.contains_key(&spec.id) {
            return Err(AraError::InvalidInput(format!(
                "duplicate model id '{}'",
                spec.id
            )));
        }
        self.models.insert(spec.id.clone(), spec);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ModelSpec> {
        self.models.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ModelSpec> {
        self.models.values()
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

/// Test fixtures. These are **synthetic** entries for unit tests only and do
/// not describe real published models.
#[doc(hidden)]
pub mod fixtures {
    use super::*;

    pub fn spec(id: &str, params_b: f32, size_mb: u64) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            display_name: format!("Test {id}"),
            version: "1".into(),
            format: ModelFormat::Gguf,
            quantization: "Q4_K_M".into(),
            parameters_b: params_b,
            file_size_bytes: size_mb * 1024 * 1024,
            kv_bytes_per_token: 64 * 1024,
            runtime_overhead_bytes: 128 * 1024 * 1024,
            max_context: 8192,
            default_context: 2048,
            languages: ["en", "ar"].iter().map(|s| s.to_string()).collect(),
            capabilities: [Capability::Chat].into_iter().collect(),
            license: "Apache-2.0".into(),
            source_url: "https://example.invalid/model.gguf".into(),
            sha256: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::spec;
    use super::*;

    #[test]
    fn json_round_trip() {
        let mut reg = ModelRegistry::new();
        reg.insert(spec("a", 1.5, 900)).unwrap();
        let mut b = spec("b", 3.0, 1800);
        b.sha256 = Some("a".repeat(64));
        reg.insert(b).unwrap();
        let back = ModelRegistry::from_json(&reg.to_json()).unwrap();
        assert_eq!(back.len(), 2);
        assert!(back.get("b").unwrap().is_checksum_verified());
        assert!(!back.get("a").unwrap().is_checksum_verified());
    }

    #[test]
    fn rejects_duplicates_and_invalid_entries() {
        let mut reg = ModelRegistry::new();
        reg.insert(spec("a", 1.0, 100)).unwrap();
        assert!(reg.insert(spec("a", 1.0, 100)).is_err());

        let mut s = spec("http", 1.0, 100);
        s.source_url = "http://insecure".into();
        assert!(reg.insert(s).is_err());

        let mut s = spec("ctx", 1.0, 100);
        s.default_context = 99999;
        assert!(reg.insert(s).is_err());

        let mut s = spec("hash", 1.0, 100);
        s.sha256 = Some("XYZ".into());
        assert!(reg.insert(s).is_err());

        let mut s = spec("bad id/..", 1.0, 100);
        s.id = "../evil".into();
        assert!(reg.insert(s).is_err());
    }

    #[test]
    fn bundled_catalog_is_valid_and_fully_verified() {
        let reg = ModelRegistry::from_json(include_str!("../../../catalog/models.json")).unwrap();
        assert!(reg.len() >= 5);
        for m in reg.iter() {
            assert!(m.is_checksum_verified(), "{} lacks sha256", m.id);
            assert!(
                m.source_url.starts_with("https://huggingface.co/"),
                "{}",
                m.id
            );
            assert!(
                ["Apache-2.0", "MIT"].contains(&m.license.as_str()),
                "{}",
                m.id
            );
        }
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let err = ModelRegistry::from_json(r#"{"schema_version":99,"models":[]}"#).unwrap_err();
        assert!(matches!(err, AraError::InvalidInput(_)));
    }

    #[test]
    fn runtime_estimate_grows_with_context() {
        let s = spec("a", 1.0, 1000);
        assert!(s.estimated_runtime_bytes(4096) > s.estimated_runtime_bytes(1024));
        assert!(s.estimated_runtime_bytes(0) >= s.file_size_bytes);
    }

    #[test]
    fn language_matching_uses_primary_subtag() {
        let s = spec("a", 1.0, 100);
        assert!(s.supports_language("ar-EG"));
        assert!(s.supports_language("EN"));
        assert!(!s.supports_language("fr"));
    }
}
