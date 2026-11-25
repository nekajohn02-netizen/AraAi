//! Unified local inference interface.
//!
//! [`InferenceEngine`] loads a model file into a [`LoadedModel`], which runs
//! streaming chat generation with cooperative cancellation and returns
//! measured [`GenerationStats`]. The llama.cpp backend lives in [`llama`]
//! (feature `llama`). Engine-independent text handling (UTF-8 reassembly,
//! stop sequences) is in [`text`] and is unit-tested without a model.

use ara_core_api::{AraResult, CancellationToken};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod text;

#[cfg(feature = "llama")]
pub mod llama;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadParams {
    /// Context window in tokens (KV cache size).
    pub context: u32,
    /// CPU threads; `None` = engine default.
    pub threads: Option<u32>,
    /// Layers to offload to GPU. 0 = CPU only (the only mode tested so far).
    pub gpu_layers: u32,
}

impl Default for LoadParams {
    fn default() -> Self {
        Self {
            context: 2048,
            threads: None,
            gpu_layers: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationParams {
    pub max_tokens: u32,
    /// 0.0 = greedy (deterministic).
    pub temperature: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub seed: u32,
    /// Generation stops before any of these strings is emitted.
    #[serde(default)]
    pub stop: Vec<String>,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            max_tokens: 256,
            temperature: 0.7,
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
            seed: 0x5eed,
            stop: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The model emitted an end-of-generation token.
    EndOfGeneration,
    MaxTokens,
    StopSequence,
    Cancelled,
    /// The context window filled up.
    ContextFull,
}

/// Measured timings for one generation call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationStats {
    pub prompt_tokens: u32,
    pub generated_tokens: u32,
    /// Time to evaluate the prompt (prefill).
    pub prompt_eval_ms: u64,
    /// From call start to the first generated token being available.
    pub time_to_first_token_ms: u64,
    /// Decode phase duration (after the first token).
    pub generation_ms: u64,
    /// Decode speed, tokens/second; 0 if fewer than 2 tokens were generated.
    pub tokens_per_sec: f32,
    pub stop_reason: StopReason,
    /// Prompt tokens served from the KV cache of the previous call (not re-evaluated).
    #[serde(default)]
    pub reused_prompt_tokens: u32,
}

pub trait InferenceEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn load(&self, path: &Path, params: &LoadParams) -> AraResult<Box<dyn LoadedModel>>;
}

pub trait LoadedModel: Send {
    /// Context window the model was loaded with.
    fn context_len(&self) -> u32;

    /// Context length the model was trained with (from file metadata).
    fn trained_context_len(&self) -> u32;

    /// Number of tokens `text` encodes to (no special tokens added).
    fn count_tokens(&self, text: &str) -> AraResult<u32>;

    /// Formats `messages` with the model's chat template and streams the
    /// reply through `on_text`. Text chunks are always valid UTF-8 and never
    /// contain (a prefix of) a stop sequence.
    fn generate_chat(
        &mut self,
        messages: &[ChatMessage],
        params: &GenerationParams,
        cancel: &CancellationToken,
        on_text: &mut dyn FnMut(&str),
    ) -> AraResult<GenerationStats>;
}

/// Decode speed helper shared by engines.
pub fn tokens_per_sec(tokens_after_first: u32, ms: u64) -> f32 {
    if tokens_after_first == 0 || ms == 0 {
        0.0
    } else {
        tokens_after_first as f32 * 1000.0 / ms as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_per_sec_handles_edge_cases() {
        assert_eq!(tokens_per_sec(0, 100), 0.0);
        assert_eq!(tokens_per_sec(10, 0), 0.0);
        assert!((tokens_per_sec(50, 2000) - 25.0).abs() < 1e-6);
    }

    #[test]
    fn params_serialize_round_trip() {
        let p = GenerationParams {
            stop: vec!["\n\n".into()],
            ..Default::default()
        };
        let back: GenerationParams =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }
}
