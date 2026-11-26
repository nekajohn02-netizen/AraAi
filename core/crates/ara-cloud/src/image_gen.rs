//! Cloud image generation (BYO-key, OpenAI-compatible `images/generations`
//! shape) — `docs/ROADMAP_TASKS.md` T2.6.
//!
//! **There is no on-device image generation in ARA, and none is planned**
//! (the project owner's explicit decision, 2026-11-25, documented in
//! `docs/EXPANSION_BLUEPRINT.md` Category Q): diffusion models are far
//! heavier than ARA's phone-RAM budget and compute-bound in a way the
//! existing memory-bandwidth-bound model selector (`ara-model-registry`,
//! ADR-003) was never designed for. This module only ever talks to a cloud
//! provider the user has already connected with their own key and money,
//! the same BYO-key pattern [`crate::CloudModel`] already uses for chat.
//!
//! **The caller must run `ara_tools::content_policy::check_media_prompt` on
//! the prompt before calling this function at all** — this module does not
//! repeat that check itself (there is exactly one place that decision
//! lives: `ara-tools::builtin::precheck("generate_image", …)`, which runs
//! before `ToolRegistry::dispatch` ever reaches a permission check, let
//! alone a network call).

use crate::{agent, map_err, short, validate_base_url, NetworkPolicy};
use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};

/// One generated image. Providers return either a hosted URL or inline
/// base64 data, never both — the caller/UI decides how to fetch/display it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub url: Option<String>,
    pub b64_json: Option<String>,
}

/// Calls `{base_url}/images/generations` (the same path shape OpenAI,
/// Together and several other providers already use for their
/// chat-completion-compatible catalogs). `size` is passed through verbatim
/// when given — `ara-tools`' own `ParamSpec::choice` already restricts it
/// to a fixed, known-reasonable set before it ever reaches here.
pub fn generate_image(
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    size: Option<&str>,
    policy: NetworkPolicy,
) -> AraResult<Vec<GeneratedImage>> {
    policy.allow()?;
    if api_key.trim().is_empty() || model.trim().is_empty() {
        return Err(AraError::InvalidInput(
            "API key and model are required".into(),
        ));
    }
    if prompt.trim().is_empty() {
        return Err(AraError::InvalidInput("prompt must not be empty".into()));
    }
    let base = validate_base_url(base_url)?;
    generate_image_at(&base, api_key, model, prompt, size)
}

/// The actual HTTP call, against an already-validated base URL. Split out
/// so tests can exercise it against a plain-`http://` local mock server
/// without weakening `generate_image`'s own https-only gate — the same
/// reasoning `ara-smarthome` documents for allowing `http://` only on its
/// local-network test paths, not the public API.
fn generate_image_at(
    base: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    size: Option<&str>,
) -> AraResult<Vec<GeneratedImage>> {
    let mut body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "n": 1,
    });
    if let Some(size) = size {
        body["size"] = serde_json::json!(size);
    }
    let resp = agent()
        .post(&format!("{base}/images/generations"))
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .map_err(map_err)?;
    let text = resp
        .into_body()
        .read_to_string()
        .map_err(|e| AraError::Io(short(&e.to_string())))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| AraError::Io("unexpected response from provider".into()))?;
    if let Some(err) = v.get("error") {
        let msg = err["message"].as_str().unwrap_or("provider error");
        return Err(AraError::Io(short(msg)));
    }
    let data = v["data"].as_array().cloned().unwrap_or_default();
    if data.is_empty() {
        return Err(AraError::Io("provider returned no image data".into()));
    }
    Ok(data
        .into_iter()
        .map(|d| GeneratedImage {
            url: d["url"].as_str().map(String::from),
            b64_json: d["b64_json"].as_str().map(String::from),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Plain-HTTP mock, same minimal shape as the one in `lib.rs`'s own
    /// tests — not shared across modules on purpose (it is 20 lines, and
    /// `ara-cloud`'s own precedent elsewhere in this crate is to duplicate
    /// a small test helper rather than add cross-module test-only coupling).
    /// Records the request body it received so a test can assert on it.
    fn mock(reply: String) -> (String, std::sync::Arc<std::sync::Mutex<String>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
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

    fn ok_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn generates_an_image_from_a_url_response_and_sends_the_expected_request() {
        let (base, seen) = mock(ok_response(
            &serde_json::json!({"data": [{"url": "https://cdn.example/x.png"}]}).to_string(),
        ));
        let out = generate_image_at(
            &base,
            "sk-test-key",
            "test-image-model",
            "a lighthouse at sunset",
            Some("512x512"),
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url.as_deref(), Some("https://cdn.example/x.png"));
        assert!(out[0].b64_json.is_none());

        let req = seen.lock().unwrap().clone();
        assert!(req.contains("POST /images/generations"));
        assert!(req.contains("Bearer sk-test-key"));
        assert!(req.contains("\"prompt\":\"a lighthouse at sunset\""));
        assert!(req.contains("\"size\":\"512x512\""));
    }

    #[test]
    fn b64_json_response_is_parsed_and_size_is_omitted_when_not_given() {
        let (base, seen) = mock(ok_response(
            &serde_json::json!({"data": [{"b64_json": "QUJD"}]}).to_string(),
        ));
        let out =
            generate_image_at(&base, "sk-test-key", "test-image-model", "a cat", None).unwrap();
        assert_eq!(out[0].b64_json.as_deref(), Some("QUJD"));
        assert!(out[0].url.is_none());
        assert!(!seen.lock().unwrap().contains("\"size\""));
    }

    #[test]
    fn provider_error_payload_becomes_a_readable_error() {
        let (base, _seen) = mock(ok_response(
            &serde_json::json!({"error": {"message": "invalid model"}}).to_string(),
        ));
        let err =
            generate_image_at(&base, "sk-test-key", "bad-model", "a cat", None).unwrap_err();
        assert!(format!("{err}").contains("invalid model"));
    }

    #[test]
    fn rejects_insecure_base_url() {
        let err = generate_image(
            "http://not-https.example",
            "sk-test",
            "test-model",
            "a lighthouse at sunset",
            None,
            NetworkPolicy::Online,
        )
        .unwrap_err();
        assert!(format!("{err}").to_lowercase().contains("https"));
    }

    #[test]
    fn rejects_empty_key_model_or_prompt() {
        assert!(generate_image(
            "https://api.example.com/v1",
            "",
            "model",
            "a cat",
            None,
            NetworkPolicy::Online
        )
        .is_err());
        assert!(generate_image(
            "https://api.example.com/v1",
            "sk-test",
            "",
            "a cat",
            None,
            NetworkPolicy::Online
        )
        .is_err());
        assert!(generate_image(
            "https://api.example.com/v1",
            "sk-test",
            "model",
            "   ",
            None,
            NetworkPolicy::Online
        )
        .is_err());
    }

    #[test]
    fn strict_offline_refuses_before_opening_any_socket() {
        // Same contract as every other `ara-cloud` call: the error must come
        // from the policy gate, not from a failed connection attempt — this
        // uses an unroutable-looking host on purpose; if the gate were
        // missing this would hang/err on DNS instead of returning instantly.
        let err = generate_image(
            "https://api.example.com/v1",
            "sk-test",
            "model",
            "a cat",
            None,
            NetworkPolicy::StrictOffline,
        )
        .unwrap_err();
        assert!(format!("{err}").to_lowercase().contains("offline"));
    }
}
