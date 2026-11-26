//! Smart-home control for devices on the same local network — through an
//! existing hub, never by Ara reimplementing a vendor's own protocol.
//!
//! **Why a hub, not a device list:** there is no single "smart device"
//! protocol — Zigbee, Z-Wave, Matter/Thread, and dozens of cloud-only vendor
//! APIs all differ. Reimplementing even a handful of them inside Ara would
//! be a permanent maintenance burden and a much bigger attack surface for a
//! privacy-focused app. [Home Assistant](https://www.home-assistant.io/) is
//! the standard self-hosted answer to exactly this problem — it already
//! speaks to thousands of device integrations — so Ara talks to *it* over
//! its documented local REST API, the same "bring your own endpoint + long-
//! lived token" shape already used for API Hub cloud providers
//! (`ara-cloud`), just pointed at the user's own LAN instead of a public
//! cloud API.
//!
//! **Deliberately not done here (see ADR-020 in `docs/ARCHITECTURE.md`):**
//! this crate is plumbing only. It is **not** registered as an
//! `ara-tools::Tool`, so the model cannot call it yet, and nothing in
//! `ara-ffi` exposes it to Kotlin. Turning a light on or off is a real-world
//! physical effect — before this is ever reachable from a chat turn it must
//! go through the exact same `External`-effect, confirm-every-time gate
//! every other tool already does (ADR-006), the same way `ara-crypto`'s
//! encryption shipped before anything called it with a real key. Building
//! that gate correctly is separate work, not rushed in alongside this.
//!
//! **Local network, not the public internet:** unlike API Hub (`ara-cloud`,
//! https-only, no exceptions), a typical self-hosted Home Assistant
//! instance on a home LAN runs over plain `http://`, with no public
//! certificate to present. Requiring https here would make this simply not
//! work for most real installs, so [`validate_base_url`] accepts either
//! scheme — but still rejects anything that is not actually a network host
//! (no `javascript:`/`file:`/etc., no embedded credentials, no query
//! string), and callers should still warn the user that an unencrypted LAN
//! connection carries the token in the clear on that network segment.

use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// One entity's current state, as Home Assistant reports it
/// (`GET /api/states`). Only the fields Ara actually uses are kept —
/// Home Assistant's real payload has many more per-entity attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityState {
    pub entity_id: String,
    pub state: String,
    pub friendly_name: Option<String>,
}

/// Validates a Home Assistant base URL: `http://` or `https://`, a real
/// host, no embedded credentials, no query/fragment — the same shape of
/// check `ara-cloud::validate_base_url` does, minus the https-only rule
/// (see module docs for why).
pub fn validate_base_url(url: &str) -> AraResult<String> {
    let u = url.trim().trim_end_matches('/');
    let rest = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
        .ok_or_else(|| AraError::InvalidInput("URL must be http:// or https://".into()))?;
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty() || host.contains('@') || u.contains('?') || u.contains('#') {
        return Err(AraError::InvalidInput("invalid Home Assistant URL".into()));
    }
    Ok(u.to_string())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .user_agent("Ara/0.1")
        .build()
        .into()
}

fn map_err(e: ureq::Error) -> AraError {
    match e {
        ureq::Error::StatusCode(401) | ureq::Error::StatusCode(403) => {
            AraError::InvalidInput("Home Assistant rejected the access token".into())
        }
        ureq::Error::StatusCode(404) => {
            AraError::NotFound("entity or service not found on this Home Assistant".into())
        }
        ureq::Error::StatusCode(c) => AraError::Io(format!("Home Assistant returned HTTP {c}")),
        other => AraError::Io(short(&other.to_string())),
    }
}

fn short(s: &str) -> String {
    s.chars().take(120).collect()
}

/// A configured Home Assistant connection. The token lives only in memory
/// for the lifetime of this value — callers are expected to keep it in
/// Android Keystore-backed storage the same way API Hub keys are, never
/// written to disk by this crate.
pub struct HomeAssistant {
    base_url: String,
    token: String,
}

impl HomeAssistant {
    pub fn new(base_url: &str, token: &str) -> AraResult<Self> {
        if token.trim().is_empty() {
            return Err(AraError::InvalidInput(
                "a long-lived access token is required".into(),
            ));
        }
        Ok(Self {
            base_url: validate_base_url(base_url)?,
            token: token.trim().to_string(),
        })
    }

    fn auth<B>(&self, req: ureq::RequestBuilder<B>) -> ureq::RequestBuilder<B> {
        req.header("Authorization", &format!("Bearer {}", self.token))
    }

    /// `GET /api/` — confirms the URL and token actually reach a running
    /// Home Assistant, without touching any device. Good as a "test
    /// connection" action before anything else is offered.
    pub fn ping(&self) -> AraResult<()> {
        self.auth(agent().get(&format!("{}/api/", self.base_url)))
            .call()
            .map_err(map_err)?;
        Ok(())
    }

    /// `GET /api/states` — every entity's current state. Read-only: safe to
    /// call without any confirmation gate (matches `ara-tools`' own
    /// Reversible/None-effect classification for read actions).
    pub fn states(&self) -> AraResult<Vec<EntityState>> {
        let resp = self
            .auth(agent().get(&format!("{}/api/states", self.base_url)))
            .call()
            .map_err(map_err)?;
        let body = resp
            .into_body()
            .read_to_string()
            .map_err(|e| AraError::Io(short(&e.to_string())))?;
        let v: Vec<serde_json::Value> = serde_json::from_str(&body)
            .map_err(|_| AraError::Io("unexpected response from Home Assistant".into()))?;
        Ok(v.into_iter()
            .filter_map(|e| {
                Some(EntityState {
                    entity_id: e.get("entity_id")?.as_str()?.to_string(),
                    state: e.get("state")?.as_str()?.to_string(),
                    friendly_name: e
                        .get("attributes")
                        .and_then(|a| a.get("friendly_name"))
                        .and_then(|n| n.as_str())
                        .map(String::from),
                })
            })
            .collect())
    }

    /// `POST /api/services/{domain}/{service}` with `{"entity_id": ...}` —
    /// e.g. `call_service("light", "turn_on", "light.kitchen")`. This is a
    /// real-world physical effect: callers MUST gate it behind the same
    /// explicit confirmation every other `External`-effect tool requires
    /// (ADR-006) — this function itself enforces no such gate, by design;
    /// it is not yet reachable from a chat turn (see module docs).
    pub fn call_service(&self, domain: &str, service: &str, entity_id: &str) -> AraResult<()> {
        // `domain`/`service` are formatted directly into the request path —
        // validated here too (not just by the caller's own precheck, e.g.
        // `ara-tools::builtin::precheck`), so this function is safe to call
        // directly and never forms a malformed or unexpected request path.
        let safe = |x: &str| {
            !x.is_empty() && x.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        };
        if !safe(domain) || !safe(service) {
            return Err(AraError::InvalidInput(
                "domain/service must be lowercase letters, digits, underscore only".into(),
            ));
        }
        let url = format!("{}/api/services/{domain}/{service}", self.base_url);
        let body = serde_json::json!({ "entity_id": entity_id }).to_string();
        self.auth(agent().post(&url))
            .header("Content-Type", "application/json")
            .send(body)
            .map_err(map_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[test]
    fn base_url_validation() {
        assert_eq!(
            validate_base_url("http://homeassistant.local:8123/").unwrap(),
            "http://homeassistant.local:8123"
        );
        assert!(validate_base_url("https://ha.example.com").is_ok());
        for bad in ["ftp://x", "https://", "https://u:p@x.com", "http://x.com?a=b", "javascript:alert(1)"] {
            assert!(validate_base_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn constructor_requires_a_token() {
        assert!(HomeAssistant::new("http://x", "").is_err());
        assert!(HomeAssistant::new("http://x", "  ").is_err());
        assert!(HomeAssistant::new("http://x", "t").is_ok());
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

    fn json_reply(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn states_parses_entities_and_sends_bearer_token() {
        let body = serde_json::json!([
            {"entity_id": "light.kitchen", "state": "on", "attributes": {"friendly_name": "Kitchen Light"}},
            {"entity_id": "sensor.temp", "state": "21.5", "attributes": {}},
        ])
        .to_string();
        let (url, seen) = mock(json_reply(&body));
        let ha = HomeAssistant::new(&url, "test-token-123").unwrap();
        let states = ha.states().unwrap();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].entity_id, "light.kitchen");
        assert_eq!(states[0].state, "on");
        assert_eq!(states[0].friendly_name.as_deref(), Some("Kitchen Light"));
        assert_eq!(states[1].friendly_name, None);
        let req = seen.lock().unwrap().clone();
        assert!(req.starts_with("GET /api/states"), "{req}");
        assert!(req.contains("Bearer test-token-123"));
    }

    #[test]
    fn call_service_posts_the_expected_path_and_body() {
        let (url, seen) = mock(json_reply("[]"));
        let ha = HomeAssistant::new(&url, "tok").unwrap();
        ha.call_service("light", "turn_on", "light.kitchen").unwrap();
        let req = seen.lock().unwrap().clone();
        assert!(req.starts_with("POST /api/services/light/turn_on"), "{req}");
        assert!(req.contains("\"entity_id\":\"light.kitchen\""));
    }

    #[test]
    fn call_service_rejects_unsafe_domain_or_service_without_any_network_call() {
        let ha = HomeAssistant::new("http://127.0.0.1:1", "tok").unwrap();
        for (domain, service) in [
            ("light; drop table", "turn_on"),
            ("light", "turn on"),
            ("../etc", "passwd"),
            ("", "turn_on"),
        ] {
            assert!(ha.call_service(domain, service, "light.x").is_err(), "{domain}/{service}");
        }
    }

    #[test]
    fn rejected_token_is_mapped_to_a_clear_error_without_leaking_it() {
        let (url, _) = mock(
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        );
        let ha = HomeAssistant::new(&url, "secret-token-xyz").unwrap();
        let err = ha.states().unwrap_err();
        assert!(err.to_string().contains("rejected the access token"));
        assert!(!err.to_string().contains("secret-token-xyz"));
    }
}
