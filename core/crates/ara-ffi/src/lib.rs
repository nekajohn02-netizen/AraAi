//! Kotlin/Android bridge (UniFFI).
//!
//! G1 decision: structured data crosses the boundary as JSON strings whose
//! schema is owned by the core crates (serde). This keeps the core crates free
//! of FFI dependencies and avoids mirroring every type. Errors are typed.
//! Streaming (chat tokens, download progress) uses UniFFI callback interfaces;
//! the stateful API is the [`core::AraCore`] object.

pub mod activity_log;
pub mod conversations;
pub mod core;

use ara_core_api::AraError;
use ara_device_profiler::HostHints;
use ara_model_registry::ModelRegistry;
use ara_model_selector::{Benchmark, ModelSelector, SelectionRequest};
use std::path::Path;

uniffi::setup_scaffolding!();

// Field is `detail`, not `message`: a `message` field clashes with
// `Throwable.message` in the generated Kotlin (found by compiling the bindings).
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    #[error("{detail}")]
    InvalidInput { detail: String },
    #[error("{detail}")]
    Unsupported { detail: String },
    #[error("{detail}")]
    Io { detail: String },
    #[error("{detail}")]
    Internal { detail: String },
    #[error("cancelled")]
    Cancelled,
}

impl From<AraError> for FfiError {
    fn from(e: AraError) -> Self {
        let detail = e.to_string();
        match e {
            AraError::InvalidInput(_) | AraError::NotFound(_) => FfiError::InvalidInput { detail },
            AraError::Unsupported(_) | AraError::InsufficientResources(_) => {
                FfiError::Unsupported { detail }
            }
            AraError::Io(_) => FfiError::Io { detail },
            AraError::Cancelled => FfiError::Cancelled,
            AraError::Internal(_) => FfiError::Internal { detail },
        }
    }
}

pub(crate) fn parse<T: serde::de::DeserializeOwned>(what: &str, json: &str) -> Result<T, FfiError> {
    serde_json::from_str(json).map_err(|e| FfiError::InvalidInput {
        detail: format!("{what}: {e}"),
    })
}

pub(crate) fn to_json<T: serde::Serialize>(v: &T) -> Result<String, FfiError> {
    serde_json::to_string(v).map_err(|e| FfiError::Internal {
        detail: e.to_string(),
    })
}

/// Version of the core library, for diagnostics.
#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns a `DeviceProfile` JSON. `hints_json` is a `HostHints` JSON
/// (may be `"{}"`), filled by Kotlin from Android APIs.
#[uniffi::export]
pub fn collect_device_profile(model_dir: String, hints_json: String) -> Result<String, FfiError> {
    let hints: HostHints = parse("hints", &hints_json)?;
    let profile = ara_device_profiler::collect(Path::new(&model_dir), hints)?;
    to_json(&profile)
}

/// Validates a catalog and returns the number of models in it.
#[uniffi::export]
pub fn validate_catalog(catalog_json: String) -> Result<u32, FfiError> {
    Ok(ModelRegistry::from_json(&catalog_json)?.len() as u32)
}

/// Returns a `SelectionResult` JSON.
#[uniffi::export]
pub fn select_models(
    catalog_json: String,
    device_json: String,
    request_json: String,
    benchmarks_json: String,
) -> Result<String, FfiError> {
    let reg = ModelRegistry::from_json(&catalog_json)?;
    let device = parse("device", &device_json)?;
    let req: SelectionRequest = parse("request", &request_json)?;
    let benches: Vec<Benchmark> = parse("benchmarks", &benchmarks_json)?;
    to_json(&ModelSelector::default().select(reg.iter(), &device, &req, &benches))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ara_model_registry::fixtures::spec;

    #[test]
    fn end_to_end_through_json_boundary() {
        let mut reg = ModelRegistry::new();
        reg.insert(spec("small", 0.5, 300)).unwrap();
        let catalog = reg.to_json();
        assert_eq!(validate_catalog(catalog.clone()).unwrap(), 1);

        let dir = std::env::temp_dir().to_string_lossy().to_string();
        let device = collect_device_profile(dir, "{}".into()).unwrap();
        let req = r#"{"mode":"automatic","capability":"chat","language":"ar","context":1024}"#;
        let out = select_models(catalog, device, req.into(), "[]".into()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v["candidates"].is_array());
        assert_eq!(v["effective_mode"], "automatic");
    }

    #[test]
    fn malformed_json_is_a_typed_error() {
        let err = select_models("{".into(), "{}".into(), "{}".into(), "[]".into()).unwrap_err();
        assert!(matches!(err, FfiError::InvalidInput { .. }));
    }
}
