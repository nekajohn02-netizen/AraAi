//! Finds smart-home/IoT devices already on the user's own local network
//! (SSDP/UPnP), so adding one to [`ara_smarthome`]'s Home Assistant link
//! doesn't require typing an IP address by hand.
//!
//! **Scope, by design:** the multicast discovery datagram this sends
//! (`M-SEARCH`) physically cannot leave the local network segment — that is
//! how IP multicast works, not a policy choice Ara could get wrong later.
//! There is deliberately no "scan the internet" or "control any device"
//! mode here: Ara only ever discovers devices already sitting on the same
//! Wi-Fi/LAN as the phone, the same way a TV-casting app finds a
//! Chromecast. Nothing here sends a command to a device; it only reads
//! the self-announcements devices already broadcast to find each other.

use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

const SSDP_MULTICAST_ADDR: &str = "239.255.255.250:1900";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    /// Where the device's UPnP description XML lives, e.g.
    /// `http://192.168.1.50:8123/description.xml`. Always a LAN address —
    /// multicast replies cannot arrive from off-network.
    pub location: String,
    /// `SERVER` header, if the device sent one.
    pub server: Option<String>,
    /// `USN`/`ST` header: the UPnP device/service type being advertised.
    pub usn: Option<String>,
    /// Best-effort label for display only — never used to decide what a
    /// device is allowed to do; actual control still goes through
    /// `ara_smarthome`'s own confirmed, Home-Assistant-mediated calls.
    pub kind_hint: DeviceKindHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKindHint {
    HomeAssistant,
    MediaRenderer,
    Router,
    Printer,
    Other,
}

impl DeviceKindHint {
    fn classify(server: Option<&str>, usn: Option<&str>) -> Self {
        let hay = format!(
            "{} {}",
            server.unwrap_or_default().to_ascii_lowercase(),
            usn.unwrap_or_default().to_ascii_lowercase()
        );
        if hay.contains("home assistant") || hay.contains("homeassistant") {
            DeviceKindHint::HomeAssistant
        } else if hay.contains("mediarenderer") || hay.contains("chromecast") || hay.contains("dial") {
            DeviceKindHint::MediaRenderer
        } else if hay.contains("internetgatewaydevice") || hay.contains("wanconnectiondevice") {
            DeviceKindHint::Router
        } else if hay.contains("printer") {
            DeviceKindHint::Printer
        } else {
            DeviceKindHint::Other
        }
    }
}

/// Parses one SSDP response datagram (HTTP/1.1-style headers, no body).
/// Returns `None` for anything without a usable `http(s)://` `LOCATION` —
/// malformed or unrelated broadcast traffic is silently ignored rather than
/// surfaced as an error, since this reads network chatter Ara doesn't
/// control the shape of.
pub fn parse_ssdp_response(raw: &str) -> Option<DiscoveredDevice> {
    let mut location = None;
    let mut server = None;
    let mut usn = None;
    for line in raw.split("\r\n") {
        let Some((key, value)) = line.split_once(':') else {
            continue; // status line, blank line, or anything else without "Key: value"
        };
        let value = value.trim().to_string();
        match key.trim().to_ascii_uppercase().as_str() {
            "LOCATION" => location = Some(value),
            "SERVER" => server = Some(value),
            "USN" | "ST" if usn.is_none() => usn = Some(value),
            _ => continue,
        }
    }
    let location = location?;
    if !location.starts_with("http://") && !location.starts_with("https://") {
        return None;
    }
    let kind_hint = DeviceKindHint::classify(server.as_deref(), usn.as_deref());
    Some(DiscoveredDevice {
        location,
        server,
        usn,
        kind_hint,
    })
}

/// Sends one SSDP discovery multicast and collects replies for `timeout`.
/// Returns an empty list — never an error — when the network is unreachable
/// (e.g. airplane mode, a sandboxed/no-multicast environment): "found
/// nothing" and "couldn't look" should not need different UI treatment.
pub fn discover_lan_devices(timeout: Duration) -> Vec<DiscoveredDevice> {
    discover_via(SSDP_MULTICAST_ADDR, timeout).unwrap_or_default()
}

fn discover_via(target: &str, timeout: Duration) -> AraResult<Vec<DiscoveredDevice>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_millis(250)))?;
    const SEARCH: &[u8] = b"M-SEARCH * HTTP/1.1\r\n\
        HOST: 239.255.255.250:1900\r\n\
        MAN: \"ssdp:discover\"\r\n\
        MX: 2\r\n\
        ST: ssdp:all\r\n\r\n";
    socket
        .send_to(SEARCH, target)
        .map_err(|e| AraError::Io(e.kind().to_string()))?;

    let deadline = Instant::now() + timeout;
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    let mut buf = [0u8; 2048];
    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, _from)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                if let Some(device) = parse_ssdp_response(&text) {
                    if seen.insert(device.location.clone()) {
                        found.push(device);
                    }
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => continue,
            Err(_) => break,
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket as StdUdp;

    #[test]
    fn parses_a_real_world_home_assistant_response() {
        let raw = "HTTP/1.1 200 OK\r\n\
            CACHE-CONTROL: max-age=1800\r\n\
            LOCATION: http://192.168.1.50:8123/description.xml\r\n\
            SERVER: Home Assistant/2024.1 UPnP/1.0\r\n\
            ST: upnp:rootdevice\r\n\
            USN: uuid:abc::upnp:rootdevice\r\n\r\n";
        let d = parse_ssdp_response(raw).unwrap();
        assert_eq!(d.location, "http://192.168.1.50:8123/description.xml");
        assert_eq!(d.kind_hint, DeviceKindHint::HomeAssistant);
    }

    #[test]
    fn parses_a_media_renderer_response() {
        let raw = "HTTP/1.1 200 OK\r\n\
            LOCATION: http://192.168.1.60:8008/ssdp/device-desc.xml\r\n\
            ST: urn:dial-multiscreen-org:service:dial:1\r\n\
            SERVER: Chromecast\r\n\r\n";
        let d = parse_ssdp_response(raw).unwrap();
        assert_eq!(d.kind_hint, DeviceKindHint::MediaRenderer);
    }

    #[test]
    fn rejects_responses_with_no_location() {
        assert!(parse_ssdp_response("HTTP/1.1 200 OK\r\nSERVER: x\r\n\r\n").is_none());
    }

    #[test]
    fn rejects_a_non_http_location_rather_than_trusting_it() {
        assert!(parse_ssdp_response("LOCATION: file:///etc/passwd\r\n\r\n").is_none());
        assert!(parse_ssdp_response("LOCATION: javascript:alert(1)\r\n\r\n").is_none());
    }

    #[test]
    fn ignores_garbage_that_is_not_header_shaped() {
        assert!(parse_ssdp_response("not an ssdp response at all").is_none());
    }

    #[test]
    fn discovery_only_talks_to_the_address_it_was_given_and_dedupes_duplicate_replies() {
        let responder = StdUdp::bind("127.0.0.1:0").unwrap();
        let responder_addr = responder.local_addr().unwrap();
        const REPLY: &[u8] =
            b"HTTP/1.1 200 OK\r\nLOCATION: http://127.0.0.1:9999/desc.xml\r\nSERVER: Test\r\n\r\n";
        let handle = std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            let (_, from) = responder.recv_from(&mut buf).unwrap();
            // Real UPnP devices commonly send more than one duplicate
            // response to a single M-SEARCH; de-duplication must hold.
            responder.send_to(REPLY, from).unwrap();
            responder.send_to(REPLY, from).unwrap();
        });
        let found = discover_via(&responder_addr.to_string(), Duration::from_millis(400)).unwrap();
        handle.join().unwrap();
        assert_eq!(found.len(), 1, "two replies for the same device must dedupe to one");
        assert_eq!(found[0].location, "http://127.0.0.1:9999/desc.xml");
    }

    #[test]
    fn no_replies_within_timeout_is_an_empty_list_not_an_error() {
        let silent = StdUdp::bind("127.0.0.1:0").unwrap();
        let addr = silent.local_addr().unwrap();
        let found = discover_via(&addr.to_string(), Duration::from_millis(80)).unwrap();
        assert!(found.is_empty());
    }
}
