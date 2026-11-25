//! API Hub: cloud models through OpenAI-compatible chat APIs.
//!
//! * The user brings their own key; the core never stores it (the app keeps
//!   it encrypted with Android Keystore and passes it per session).
//! * [`CloudModel`] implements [`LoadedModel`], so personas, RAG, tools and
//!   confirmations work exactly as with local models.
//! * Every outgoing message passes the [`shield::Shield`]; the reply is
//!   restored on the phone. Keys never appear in errors.
//! * https only; Kimi (Moonshot), DeepSeek, OpenAI, OpenRouter, Groq, Mistral,
//!   Together presets, or any compatible endpoint.

pub mod shield;

use ara_core_api::{AraError, AraResult, CancellationToken};
use ara_inference::{ChatMessage, GenerationParams, GenerationStats, LoadedModel, StopReason};
use serde::{Deserialize, Serialize};
use shield::Shield;
use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    pub key_url: &'static str,
    pub note: &'static str,
}

pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "moonshot",
        name: "Kimi (Moonshot AI)",
        base_url: "https://api.moonshot.ai/v1",
        key_url: "https://platform.moonshot.ai",
        note: "Kimi K3 / K2 family; very large models that cannot run on a phone.",
    },
    ProviderPreset {
        id: "deepseek",
        name: "DeepSeek",
        base_url: "https://api.deepseek.com/v1",
        key_url: "https://platform.deepseek.com",
        note: "Strong reasoning and coding models.",
    },
    ProviderPreset {
        id: "openai",
        name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        key_url: "https://platform.openai.com",
        note: "",
    },
    ProviderPreset {
        id: "openrouter",
        name: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1",
        key_url: "https://openrouter.ai/keys",
        note: "One key, many providers.",
    },
    ProviderPreset {
        id: "groq",
        name: "Groq",
        base_url: "https://api.groq.com/openai/v1",
        key_url: "https://console.groq.com",
        note: "Very fast inference.",
    },
    ProviderPreset {
        id: "mistral",
        name: "Mistral AI",
        base_url: "https://api.mistral.ai/v1",
        key_url: "https://console.mistral.ai",
        note: "",
    },
    ProviderPreset {
        id: "together",
        name: "Together AI",
        base_url: "https://api.together.xyz/v1",
        key_url: "https://api.together.ai",
        note: "",
    },
];

/// Validates a base URL: https, no credentials, no query/fragment.
pub fn validate_base_url(url: &str) -> AraResult<String> {
    let u = url.trim().trim_end_matches('/');
    let rest = u
        .strip_prefix("https://")
        .ok_or_else(|| AraError::InvalidInput("provider URL must use https".into()))?;
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty()
        || host.contains('@')
        || u.contains('?')
        || u.contains('#')
        || u.contains(' ')
    {
        return Err(AraError::InvalidInput("invalid provider URL".into()));
    }
    Ok(u.to_string())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(90)))
        .user_agent("ARA/0.1")
        .build()
        .into()
}

fn map_err(e: ureq::Error) -> AraError {
    match e {
        ureq::Error::StatusCode(401) | ureq::Error::StatusCode(403) => {
            AraError::InvalidInput("the provider rejected the API key".into())
        }
        ureq::Error::StatusCode(404) => {
            AraError::NotFound("model or endpoint not found at provider".into())
        }
        ureq::Error::StatusCode(429) => {
            AraError::InsufficientResources("provider rate limit or quota reached".into())
        }
        ureq::Error::StatusCode(c) => AraError::Io(format!("provider returned HTTP {c}")),
        other => AraError::Io(format!("network error: {}", short(&other.to_string()))),
    }
}

fn short(s: &str) -> String {
    s.chars().take(120).collect()
}

/// Lists model ids (GET /models) — also serves as "test connection".
pub fn list_models(base_url: &str, api_key: &str) -> AraResult<Vec<String>> {
    let base = validate_base_url(base_url)?;
    let resp = agent()
        .get(&format!("{base}/models"))
        .header("Authorization", &format!("Bearer {api_key}"))
        .call()
        .map_err(map_err)?;
    let body = resp
        .into_body()
        .read_to_string()
        .map_err(|e| AraError::Io(short(&e.to_string())))?;
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| AraError::Io("unexpected response from provider".into()))?;
    let mut ids: Vec<String> = v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    Ok(ids)
}

/// A cloud model session. Holds the key only in memory.
pub struct CloudModel {
    base_url: String,
    api_key: String,
    model: String,
    context: u32,
    shield_enabled: bool,
    /// Placeholders created so far (inspectable for the UI).
    pub last_redactions: usize,
}

impl CloudModel {
    pub fn new(
        base_url: &str,
        api_key: &str,
        model: &str,
        context: u32,
        shield: bool,
    ) -> AraResult<Self> {
        if api_key.trim().is_empty() || model.trim().is_empty() {
            return Err(AraError::InvalidInput(
                "API key and model are required".into(),
            ));
        }
        Ok(Self {
            base_url: validate_base_url(base_url)?,
            api_key: api_key.trim().to_string(),
            model: model.trim().to_string(),
            context: context.clamp(1024, 131_072),
            shield_enabled: shield,
            last_redactions: 0,
        })
    }
}

impl std::fmt::Debug for CloudModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("CloudModel")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .finish()
    }
}

impl LoadedModel for CloudModel {
    fn context_len(&self) -> u32 {
        self.context
    }
    fn trained_context_len(&self) -> u32 {
        self.context
    }
    /// Approximation (no remote tokenizer): ~3 chars per token for mixed
    /// Arabic/English, rounded up — conservative for context budgeting.
    fn count_tokens(&self, text: &str) -> AraResult<u32> {
        Ok((text.chars().count() as u32).div_ceil(3))
    }

    fn generate_chat(
        &mut self,
        messages: &[ChatMessage],
        params: &GenerationParams,
        cancel: &CancellationToken,
        on_text: &mut dyn FnMut(&str),
    ) -> AraResult<GenerationStats> {
        let started = Instant::now();
        let mut shield = Shield::new();
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                let content = if self.shield_enabled {
                    shield.redact(&m.content)
                } else {
                    m.content.clone()
                };
                serde_json::json!({ "role": m.role.as_str(), "content": content })
            })
            .collect();
        self.last_redactions = shield.redacted_count();
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": msgs,
            "stream": true,
            "max_tokens": params.max_tokens,
            "temperature": params.temperature,
            "top_p": params.top_p,
        });
        if !params.stop.is_empty() {
            body["stop"] = serde_json::json!(params.stop.iter().take(4).collect::<Vec<_>>());
        }
        let resp = agent()
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .send(body.to_string())
            .map_err(map_err)?;
        let reader = BufReader::new(resp.into_body().into_reader());
        let mut first: Option<Instant> = None;
        let mut pieces = 0u32;
        let mut reason = StopReason::EndOfGeneration;
        let mut pending = String::new();
        for line in reader.lines() {
            if cancel.is_cancelled() {
                reason = StopReason::Cancelled;
                break; // dropping the reader closes the connection
            }
            let line =
                line.map_err(|e| AraError::Io(format!("stream interrupted: {}", e.kind())))?;
            let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                continue;
            };
            if data == "[DONE]" {
                break;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            if let Some(err) = v.get("error") {
                let msg = err["message"].as_str().unwrap_or("provider error");
                return Err(AraError::Io(short(msg)));
            }
            let choice = &v["choices"][0];
            if let Some(t) = choice["delta"]["content"].as_str() {
                if !t.is_empty() {
                    first.get_or_insert_with(Instant::now);
                    pieces += 1;
                    let shown = shield.restore_stream(&mut pending, t);
                    if !shown.is_empty() {
                        on_text(&shown);
                    }
                }
            }
            match choice["finish_reason"].as_str() {
                Some("length") => reason = StopReason::MaxTokens,
                Some("stop") => reason = StopReason::EndOfGeneration,
                _ => {}
            }
        }
        if reason != StopReason::Cancelled && !pending.is_empty() {
            on_text(&shield.restore(&pending));
        }
        let first = first.unwrap_or_else(Instant::now);
        let generation_ms = first.elapsed().as_millis() as u64;
        Ok(GenerationStats {
            prompt_tokens: 0,
            generated_tokens: pieces,
            prompt_eval_ms: 0,
            time_to_first_token_ms: first.duration_since(started).as_millis() as u64,
            generation_ms,
            tokens_per_sec: ara_inference::tokens_per_sec(pieces.saturating_sub(1), generation_ms),
            stop_reason: reason,
            reused_prompt_tokens: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ara_inference::Role;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[test]
    fn base_url_validation() {
        assert_eq!(
            validate_base_url("https://api.moonshot.ai/v1/").unwrap(),
            "https://api.moonshot.ai/v1"
        );
        for bad in [
            "http://x.com/v1",
            "https://",
            "https://u:p@x.com",
            "https://x.com/v1?a=b",
            "ftp://x",
        ] {
            assert!(validate_base_url(bad).is_err(), "{bad}");
        }
        for p in PRESETS {
            validate_base_url(p.base_url).unwrap();
        }
    }

    /// Plain-HTTP mock that records the request and replies with `reply`.
    fn mock(reply: String) -> (String, Arc<Mutex<String>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(String::new()));
        let s2 = seen.clone();
        std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            let mut buf = vec![0u8; 65536];
            let mut req = Vec::new();
            loop {
                let n = c.read(&mut buf).unwrap();
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if req.len() >= h + 4 + len {
                        break;
                    }
                }
            }
            *s2.lock().unwrap() = String::from_utf8_lossy(&req).to_string();
            let _ = c.write_all(reply.as_bytes());
        });
        (url, seen)
    }

    fn sse(chunks: &[&str]) -> String {
        let mut body = String::new();
        for c in chunks {
            body.push_str(&format!(
                "data: {}\n\n",
                serde_json::json!({"choices":[{"delta":{"content": c}}]})
            ));
        }
        body.push_str(
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
        );
        format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
    }

    /// Test-only constructor that allows the plain-http mock.
    fn model_for(url: &str) -> CloudModel {
        CloudModel {
            base_url: url.to_string(),
            api_key: "sk-test-key-123456789".into(),
            model: "kimi-test".into(),
            context: 8192,
            shield_enabled: true,
            last_redactions: 0,
        }
    }

    #[test]
    fn streams_redacts_outgoing_and_restores_incoming() {
        let (url, seen) = mock(sse(&["هكلم ", "[PHO", "NE_1] حالًا"]));
        let mut m = model_for(&url);
        let mut out = String::new();
        let st = m
            .generate_chat(
                &[ChatMessage::new(
                    Role::User,
                    "كلم 01001234567 وقوله أنا جاي",
                )],
                &GenerationParams::default(),
                &CancellationToken::new(),
                &mut |t| out.push_str(t),
            )
            .unwrap();
        assert_eq!(out, "هكلم 01001234567 حالًا");
        assert_eq!(st.stop_reason, StopReason::EndOfGeneration);
        let req = seen.lock().unwrap().clone();
        assert!(req.starts_with("POST /chat/completions"), "{req}");
        assert!(req.contains("Bearer sk-test-key-123456789"));
        assert!(
            !req.contains("01001234567"),
            "phone must not leave the device"
        );
        assert!(req.contains("[PHONE_1]") && req.contains("\"stream\":true"));
        assert_eq!(m.last_redactions, 1);
        assert!(!format!("{m:?}").contains("sk-test"), "Debug hides the key");
    }

    #[test]
    fn http_errors_are_mapped_without_leaking_the_key() {
        let (url, _) = mock(
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        );
        let mut m = model_for(&url);
        let e = m
            .generate_chat(
                &[ChatMessage::new(Role::User, "hi")],
                &GenerationParams::default(),
                &CancellationToken::new(),
                &mut |_| {},
            )
            .unwrap_err();
        assert!(e.to_string().contains("rejected the API key"));
        assert!(!e.to_string().contains("sk-test"));
    }

    #[test]
    fn cancellation_stops_the_stream() {
        let (url, _) = mock(sse(&["a", "b", "c", "d"]));
        let mut m = model_for(&url);
        let cancel = CancellationToken::new();
        let c2 = cancel.clone();
        let st = m
            .generate_chat(
                &[ChatMessage::new(Role::User, "hi")],
                &GenerationParams::default(),
                &cancel,
                &mut |_| c2.cancel(),
            )
            .unwrap();
        assert_eq!(st.stop_reason, StopReason::Cancelled);
    }

    #[test]
    fn constructor_validates() {
        assert!(CloudModel::new("http://x.com", "k", "m", 8192, true).is_err());
        assert!(CloudModel::new("https://x.com/v1", "", "m", 8192, true).is_err());
        assert!(CloudModel::new("https://x.com/v1", "k", "m", 8192, true).is_ok());
    }
}
