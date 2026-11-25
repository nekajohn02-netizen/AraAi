//! Built-in tools.
//!
//! Core tools run here (pure computation). Host tools are executed by the
//! Android app through standard intents that open the target app with the
//! content pre-filled — the user presses "send" there. ARA never sends a
//! message, posts or places a call silently.

use crate::schema::ParamSpec;
use crate::{Permission, Runtime, SideEffect, ToolCall, ToolManifest};
use ara_core_api::{AraError, AraResult};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn manifest(
    id: &str,
    description: &str,
    params: Vec<(&str, ParamSpec)>,
    permissions: Vec<Permission>,
    side_effect: SideEffect,
    runtime: Runtime,
) -> ToolManifest {
    ToolManifest {
        id: id.into(),
        version: 1,
        description: description.into(),
        params: params
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<BTreeMap<_, _>>(),
        permissions,
        side_effect,
        runtime,
        timeout_ms: if runtime == Runtime::Core {
            500
        } else {
            10_000
        },
        max_output_bytes: 4096,
        rate_per_minute: if side_effect == SideEffect::External {
            6
        } else {
            30
        },
    }
}

const TEXT_MAX: u32 = 4000;

pub fn core_manifests() -> Vec<ToolManifest> {
    use Runtime::Core;
    use SideEffect::None as Pure;
    vec![
        manifest(
            "calculator",
            "Evaluate an arithmetic expression: + - * / % ^ ( ), sqrt sin cos tan ln log10 abs round floor ceil, pi, e.",
            vec![("expression", ParamSpec::string(200, true, "e.g. (12.5*4)^2/3"))],
            vec![],
            Pure,
            Core,
        ),
        manifest(
            "unit_convert",
            "Convert a value between units of length, mass, temperature, volume or data size.",
            vec![
                ("value", ParamSpec::number(-1e15, 1e15, true, "amount")),
                ("from", ParamSpec::choice(UNITS, true, "source unit")),
                ("to", ParamSpec::choice(UNITS, true, "target unit")),
            ],
            vec![],
            Pure,
            Core,
        ),
        manifest(
            "date_calc",
            "Date math on ISO dates (YYYY-MM-DD): weekday, days_between, add_days.",
            vec![
                ("op", ParamSpec::choice(&["weekday", "days_between", "add_days"], true, "operation")),
                ("date", ParamSpec::string(10, true, "YYYY-MM-DD")),
                ("other_date", ParamSpec::string(10, false, "YYYY-MM-DD for days_between")),
                ("days", ParamSpec::int(-100_000, 100_000, false, "for add_days")),
            ],
            vec![],
            Pure,
            Core,
        ),
        manifest(
            "text_stats",
            "Count characters, words and lines of a text and estimate reading time.",
            vec![("text", ParamSpec::string(20_000, true, "text"))],
            vec![],
            Pure,
            Core,
        ),
    ]
}

/// Tools executed by the embedding layer (they need ARA's local state).
pub fn service_manifests() -> Vec<ToolManifest> {
    vec![
        manifest(
            "search_documents",
            "Search the user's imported files. Returns passages with file name and line to cite.",
            vec![(
                "query",
                ParamSpec::string(300, true, "keywords to look for"),
            )],
            vec![Permission::ReadUserFiles],
            SideEffect::None,
            Runtime::Service,
        ),
        manifest(
            "remember",
            "Save a short fact about the user to long-term memory (the user approves first).",
            vec![(
                "fact",
                ParamSpec::string(300, true, "the fact, one sentence"),
            )],
            vec![Permission::Memory],
            SideEffect::Reversible,
            Runtime::Service,
        ),
    ]
}

pub const APPS: &[&str] = &[
    "whatsapp",
    "telegram",
    "camera",
    "gallery",
    "settings",
    "maps",
    "browser",
    "phone",
    "messages",
    "calendar",
    "clock",
    "youtube",
    "facebook",
    "instagram",
    "x",
    "tiktok",
    "email",
    "files",
    "calculator",
    "play_store",
];

pub fn host_manifests() -> Vec<ToolManifest> {
    use Permission::*;
    use Runtime::Host;
    use SideEffect::{External, Reversible};
    vec![
        manifest(
            "open_app",
            "Open an app on the phone.",
            vec![("app", ParamSpec::choice(APPS, true, "app to open"))],
            vec![LaunchApps],
            Reversible,
            Host,
        ),
        manifest(
            "whatsapp_compose",
            "Open WhatsApp with a message ready to send (the user presses send).",
            vec![
                ("text", ParamSpec::string(TEXT_MAX, true, "message")),
                ("phone", ParamSpec::string(20, false, "international number, e.g. +201234567890")),
            ],
            vec![ComposeMessages],
            External,
            Host,
        ),
        manifest(
            "telegram_compose",
            "Open Telegram with a message ready to send to a chosen chat.",
            vec![("text", ParamSpec::string(TEXT_MAX, true, "message"))],
            vec![ComposeMessages],
            External,
            Host,
        ),
        manifest(
            "sms_compose",
            "Open the SMS app with a message ready to send.",
            vec![
                ("text", ParamSpec::string(TEXT_MAX, true, "message")),
                ("phone", ParamSpec::string(20, false, "recipient number")),
            ],
            vec![ComposeMessages],
            External,
            Host,
        ),
        manifest(
            "email_compose",
            "Open the email app with a drafted email.",
            vec![
                ("to", ParamSpec::string(254, false, "recipient address")),
                ("subject", ParamSpec::string(200, true, "subject")),
                ("body", ParamSpec::string(TEXT_MAX, true, "body")),
            ],
            vec![ComposeMessages],
            External,
            Host,
        ),
        manifest(
            "share_text",
            "Share text to a social app (Facebook, Instagram, X, LinkedIn, TikTok) or any app the user picks.",
            vec![
                ("text", ParamSpec::string(TEXT_MAX, true, "post text")),
                (
                    "target",
                    ParamSpec::choice(
                        &["any", "facebook", "instagram", "x", "linkedin", "tiktok"],
                        true,
                        "target app",
                    ),
                ),
            ],
            vec![ShareContent],
            External,
            Host,
        ),
        manifest(
            "dial_number",
            "Open the phone dialer with a number filled in (the user presses call).",
            vec![("phone", ParamSpec::string(20, true, "number"))],
            vec![PhoneDialer],
            External,
            Host,
        ),
        manifest(
            "set_alarm",
            "Set a wake-up alarm at a clock time (e.g. wake me at 6:30). Use this for any alarm.",
            vec![
                ("hour", ParamSpec::int(0, 23, true, "0-23")),
                ("minute", ParamSpec::int(0, 59, true, "0-59")),
                ("label", ParamSpec::string(60, false, "label")),
            ],
            vec![Alarms],
            Reversible,
            Host,
        ),
        manifest(
            "set_timer",
            "Start a countdown timer for a duration (e.g. 10 minutes).",
            vec![
                ("minutes", ParamSpec::int(0, 1440, false, "whole minutes")),
                ("seconds", ParamSpec::int(0, 59, false, "extra seconds")),
                ("label", ParamSpec::string(60, false, "label")),
            ],
            vec![Alarms],
            Reversible,
            Host,
        ),
        manifest(
            "calendar_event",
            "Add a meeting or appointment with a title on a specific date to the calendar. Not for alarms.",
            vec![
                ("title", ParamSpec::string(200, true, "title")),
                ("start", ParamSpec::string(16, true, "local time YYYY-MM-DDTHH:MM")),
                ("duration_minutes", ParamSpec::int(1, 10_080, false, "default 60")),
                ("location", ParamSpec::string(200, false, "place")),
            ],
            vec![Calendar],
            Reversible,
            Host,
        ),
        manifest(
            "open_url",
            "Open an https web page in the browser.",
            vec![("url", ParamSpec::string(2048, true, "https URL"))],
            vec![OpenWeb],
            Reversible,
            Host,
        ),
        manifest(
            "web_search",
            "Search the web in the browser.",
            vec![("query", ParamSpec::string(300, true, "search terms"))],
            vec![OpenWeb],
            Reversible,
            Host,
        ),
        manifest(
            "maps_search",
            "Search a place or get directions in the maps app.",
            vec![("query", ParamSpec::string(300, true, "place or address"))],
            vec![OpenWeb],
            Reversible,
            Host,
        ),
        manifest(
            "copy_to_clipboard",
            "Copy text to the clipboard.",
            vec![("text", ParamSpec::string(TEXT_MAX, true, "text"))],
            vec![Clipboard],
            Reversible,
            Host,
        ),
        manifest(
            "flashlight",
            "Turn the flashlight on or off.",
            vec![("on", ParamSpec::boolean(true, "true = on"))],
            vec![DeviceControls],
            Reversible,
            Host,
        ),
        manifest(
            "open_settings",
            "Open a system settings screen.",
            vec![(
                "section",
                ParamSpec::choice(
                    &["wifi", "bluetooth", "display", "sound", "battery", "apps", "location", "notifications", "storage"],
                    true,
                    "settings section",
                ),
            )],
            vec![DeviceControls],
            Reversible,
            Host,
        ),
    ]
}

/// Semantic checks beyond the schema (formats the schema can't express).
pub fn precheck(tool: &str, args: &Value) -> Result<(), String> {
    let s = |k: &str| args.get(k).and_then(Value::as_str);
    if let Some(p) = s("phone") {
        let digits = p.strip_prefix('+').unwrap_or(p);
        if digits.len() < 3 || !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err("phone must be digits with optional leading +".into());
        }
    }
    match tool {
        "open_url" => {
            let u = s("url").unwrap_or("");
            let host = u.strip_prefix("https://").unwrap_or("");
            if host.is_empty() || host.starts_with('/') || u.chars().any(char::is_whitespace) {
                return Err("only https URLs are allowed".into());
            }
        }
        "set_timer" => {
            let total = args.get("minutes").and_then(Value::as_i64).unwrap_or(0) * 60
                + args.get("seconds").and_then(Value::as_i64).unwrap_or(0);
            if total <= 0 {
                return Err("timer needs minutes or seconds".into());
            }
        }
        "email_compose" => {
            if let Some(to) = s("to") {
                if !to.contains('@') || to.chars().any(char::is_whitespace) {
                    return Err("invalid email address".into());
                }
            }
        }
        "calendar_event" => {
            let st = s("start").unwrap_or("");
            let ok = st.len() == 16
                && parse_date(&st[..10]).is_ok()
                && &st[10..11] == "T"
                && st[11..13].parse::<u32>().is_ok_and(|h| h < 24)
                && &st[13..14] == ":"
                && st[14..16].parse::<u32>().is_ok_and(|m| m < 60);
            if !ok {
                return Err("start must be YYYY-MM-DDTHH:MM".into());
            }
        }
        _ => {}
    }
    Ok(())
}

/// One-line description of what will happen, shown in the confirmation UI.
pub fn summarize(m: &ToolManifest, args: &Value) -> String {
    let s = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("");
    let i = |k: &str| args.get(k).and_then(Value::as_i64).unwrap_or(0);
    let short = |t: &str| {
        let t: String = t.chars().take(120).collect();
        format!("«{t}»")
    };
    match m.id.as_str() {
        "whatsapp_compose" => format!("WhatsApp → {}: {}", or_pick(s("phone")), short(s("text"))),
        "telegram_compose" => format!("Telegram → (choose chat): {}", short(s("text"))),
        "sms_compose" => format!("SMS → {}: {}", or_pick(s("phone")), short(s("text"))),
        "email_compose" => format!("Email → {}: {}", or_pick(s("to")), short(s("subject"))),
        "share_text" => format!("Share → {}: {}", s("target"), short(s("text"))),
        "dial_number" => format!("Dialer → {}", s("phone")),
        "set_alarm" => format!("⏰ {:02}:{:02} {}", i("hour"), i("minute"), s("label"))
            .trim_end()
            .to_string(),
        "set_timer" => format!("⏱ {} min {} s {}", i("minutes"), i("seconds"), s("label"))
            .trim_end()
            .to_string(),
        "calendar_event" => format!("📅 {} — {} ({} min)", s("title"), s("start"), {
            let d = i("duration_minutes");
            if d == 0 {
                60
            } else {
                d
            }
        }),
        "flashlight" => format!(
            "🔦 {}",
            if args.get("on").and_then(Value::as_bool) == Some(true) {
                "ON"
            } else {
                "OFF"
            }
        ),
        "open_app" => format!("Open app: {}", s("app")),
        "open_settings" => format!("Open settings: {}", s("section")),
        "open_url" => format!("Open: {}", s("url")),
        "web_search" => format!("Web search: {}", short(s("query"))),
        "maps_search" => format!("Maps: {}", short(s("query"))),
        "copy_to_clipboard" => format!("Copy: {}", short(s("text"))),
        "remember" => format!("🧠 {}", short(s("fact"))),
        _ => format!("{}: {}", m.id, args),
    }
}

fn or_pick(v: &str) -> &str {
    if v.is_empty() {
        "(choose contact)"
    } else {
        v
    }
}

pub fn run_core(call: &ToolCall) -> AraResult<Value> {
    let a = &call.arguments;
    let s = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("");
    match call.tool.as_str() {
        "calculator" => Ok(json!({"result": calc::eval(s("expression"))?})),
        "unit_convert" => {
            let v = a.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            Ok(json!({"result": convert(v, s("from"), s("to"))?, "unit": s("to")}))
        }
        "date_calc" => date_calc(a),
        "text_stats" => {
            let t = s("text");
            let words = t.split_whitespace().count();
            Ok(json!({
                "characters": t.chars().count(),
                "words": words,
                "lines": t.lines().count(),
                "reading_minutes": (words as f64 / 200.0 * 10.0).ceil() / 10.0,
            }))
        }
        other => Err(AraError::NotFound(format!("core tool '{other}'"))),
    }
}

// ---------------------------------------------------------------- units

const UNITS: &[&str] = &[
    "m", "km", "cm", "mm", "mi", "ft", "in", "yd", "kg", "g", "mg", "lb", "oz", "c", "f", "k", "l",
    "ml", "gal", "cup", "b", "kb", "mb", "gb", "tb",
];

fn unit_factor(u: &str) -> Option<(&'static str, f64)> {
    Some(match u {
        "m" => ("len", 1.0),
        "km" => ("len", 1000.0),
        "cm" => ("len", 0.01),
        "mm" => ("len", 0.001),
        "mi" => ("len", 1609.344),
        "ft" => ("len", 0.3048),
        "in" => ("len", 0.0254),
        "yd" => ("len", 0.9144),
        "kg" => ("mass", 1.0),
        "g" => ("mass", 0.001),
        "mg" => ("mass", 1e-6),
        "lb" => ("mass", 0.453_592_37),
        "oz" => ("mass", 0.028_349_523_125),
        "l" => ("vol", 1.0),
        "ml" => ("vol", 0.001),
        "gal" => ("vol", 3.785_411_784),
        "cup" => ("vol", 0.236_588_236_5),
        "b" => ("data", 1.0),
        "kb" => ("data", 1024.0),
        "mb" => ("data", 1024f64.powi(2)),
        "gb" => ("data", 1024f64.powi(3)),
        "tb" => ("data", 1024f64.powi(4)),
        _ => return None,
    })
}

fn convert(v: f64, from: &str, to: &str) -> AraResult<f64> {
    let temp = |u: &str| matches!(u, "c" | "f" | "k");
    if temp(from) || temp(to) {
        if !(temp(from) && temp(to)) {
            return Err(AraError::InvalidInput("incompatible units".into()));
        }
        let c = match from {
            "c" => v,
            "f" => (v - 32.0) * 5.0 / 9.0,
            _ => v - 273.15,
        };
        return Ok(match to {
            "c" => c,
            "f" => c * 9.0 / 5.0 + 32.0,
            _ => c + 273.15,
        });
    }
    match (unit_factor(from), unit_factor(to)) {
        (Some((a, fa)), Some((b, fb))) if a == b => Ok(v * fa / fb),
        _ => Err(AraError::InvalidInput("incompatible units".into())),
    }
}

// ---------------------------------------------------------------- dates

/// Days since 1970-01-01 (proleptic Gregorian), H. Hinnant's algorithm.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn parse_date(s: &str) -> AraResult<i64> {
    let bad = || AraError::InvalidInput(format!("invalid date '{s}', expected YYYY-MM-DD"));
    let p: Vec<&str> = s.split('-').collect();
    if p.len() != 3 || p[0].len() != 4 || p[1].len() != 2 || p[2].len() != 2 {
        return Err(bad());
    }
    let (y, m, d) = (
        p[0].parse::<i64>().map_err(|_| bad())?,
        p[1].parse::<u32>().map_err(|_| bad())?,
        p[2].parse::<u32>().map_err(|_| bad())?,
    );
    if !(1..=12).contains(&m) || d == 0 {
        return Err(bad());
    }
    let days = days_from_civil(y, m, d);
    // Round-trip rejects 2023-02-30 etc.
    if civil_from_days(days) != (y, m, d) {
        return Err(bad());
    }
    Ok(days)
}

fn date_calc(a: &Value) -> AraResult<Value> {
    let s = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("");
    let d = parse_date(s("date"))?;
    const NAMES: [&str; 7] = [
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
    ];
    match s("op") {
        "weekday" => Ok(json!({"weekday": NAMES[d.rem_euclid(7) as usize]})),
        "days_between" => Ok(json!({"days": parse_date(s("other_date"))? - d})),
        "add_days" => {
            let n = a.get("days").and_then(Value::as_i64).unwrap_or(0);
            let (y, m, dd) = civil_from_days(d + n);
            Ok(json!({"date": format!("{y:04}-{m:02}-{dd:02}")}))
        }
        _ => Err(AraError::InvalidInput("unknown op".into())),
    }
}

// ---------------------------------------------------------------- calculator

mod calc {
    //! Recursive-descent evaluator. No `eval`, bounded depth and length.
    use ara_core_api::{AraError, AraResult};

    const MAX_DEPTH: u32 = 32;

    pub fn eval(src: &str) -> AraResult<f64> {
        let mut p = P {
            s: src.as_bytes(),
            i: 0,
            depth: 0,
        };
        let v = p.expr()?;
        p.ws();
        if p.i != p.s.len() {
            return Err(err("unexpected input"));
        }
        if !v.is_finite() {
            return Err(err("result is not a finite number"));
        }
        // Avoid 0.1+0.2 = 0.30000000000000004 surprises in answers.
        Ok((v * 1e12).round() / 1e12)
    }

    fn err(m: &str) -> AraError {
        AraError::InvalidInput(format!("calculator: {m}"))
    }

    struct P<'a> {
        s: &'a [u8],
        i: usize,
        depth: u32,
    }

    impl P<'_> {
        fn ws(&mut self) {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
        }
        fn peek(&mut self) -> Option<u8> {
            self.ws();
            self.s.get(self.i).copied()
        }
        fn expr(&mut self) -> AraResult<f64> {
            self.depth += 1;
            if self.depth > MAX_DEPTH {
                return Err(err("expression too deep"));
            }
            let mut v = self.term()?;
            while let Some(c @ (b'+' | b'-')) = self.peek() {
                self.i += 1;
                let r = self.term()?;
                v = if c == b'+' { v + r } else { v - r };
            }
            self.depth -= 1;
            Ok(v)
        }
        fn term(&mut self) -> AraResult<f64> {
            let mut v = self.power()?;
            while let Some(c @ (b'*' | b'/' | b'%')) = self.peek() {
                self.i += 1;
                let r = self.power()?;
                v = match c {
                    b'*' => v * r,
                    _ if r == 0.0 => return Err(err("division by zero")),
                    b'/' => v / r,
                    _ => v % r,
                };
            }
            Ok(v)
        }
        fn power(&mut self) -> AraResult<f64> {
            let base = self.unary()?;
            if self.peek() == Some(b'^') {
                self.i += 1;
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return Err(err("expression too deep"));
                }
                let e = self.power()?; // right-associative
                self.depth -= 1;
                return Ok(base.powf(e));
            }
            Ok(base)
        }
        fn unary(&mut self) -> AraResult<f64> {
            match self.peek() {
                Some(b'-') => {
                    self.i += 1;
                    self.depth += 1;
                    if self.depth > MAX_DEPTH {
                        return Err(err("expression too deep"));
                    }
                    let v = -self.unary()?;
                    self.depth -= 1;
                    Ok(v)
                }
                Some(b'+') => {
                    self.i += 1;
                    self.unary()
                }
                _ => self.atom(),
            }
        }
        fn atom(&mut self) -> AraResult<f64> {
            match self.peek() {
                Some(b'(') => {
                    self.i += 1;
                    let v = self.expr()?;
                    if self.peek() != Some(b')') {
                        return Err(err("missing ')'"));
                    }
                    self.i += 1;
                    Ok(v)
                }
                Some(c) if c.is_ascii_digit() || c == b'.' => {
                    let st = self.i;
                    while self.i < self.s.len()
                        && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.')
                    {
                        self.i += 1;
                    }
                    std::str::from_utf8(&self.s[st..self.i])
                        .unwrap()
                        .parse()
                        .map_err(|_| err("bad number"))
                }
                Some(c) if c.is_ascii_alphabetic() => {
                    let st = self.i;
                    while self.i < self.s.len() && self.s[self.i].is_ascii_alphanumeric() {
                        self.i += 1;
                    }
                    let name = std::str::from_utf8(&self.s[st..self.i]).unwrap();
                    match name {
                        "pi" => return Ok(std::f64::consts::PI),
                        "e" => return Ok(std::f64::consts::E),
                        _ => {}
                    }
                    if self.peek() != Some(b'(') {
                        return Err(err("unknown name"));
                    }
                    let x = self.atom()?;
                    Ok(match name {
                        "sqrt" if x < 0.0 => return Err(err("sqrt of negative")),
                        "sqrt" => x.sqrt(),
                        "sin" => x.sin(),
                        "cos" => x.cos(),
                        "tan" => x.tan(),
                        "ln" => x.ln(),
                        "log10" => x.log10(),
                        "abs" => x.abs(),
                        "round" => x.round(),
                        "floor" => x.floor(),
                        "ceil" => x.ceil(),
                        _ => return Err(err("unknown function")),
                    })
                }
                _ => Err(err("expected a number")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(tool: &str, args: Value) -> AraResult<Value> {
        run_core(&ToolCall {
            tool: tool.into(),
            arguments: args,
        })
    }

    #[test]
    fn calculator_handles_precedence_functions_and_errors() {
        assert_eq!(calc::eval("2+3*4").unwrap(), 14.0);
        assert_eq!(calc::eval("2^3^2").unwrap(), 512.0);
        assert_eq!(calc::eval("-(2+3)*2").unwrap(), -10.0);
        assert_eq!(calc::eval("0.1+0.2").unwrap(), 0.3);
        assert_eq!(calc::eval("sqrt(16)+abs(-2)").unwrap(), 6.0);
        assert!((calc::eval("cos(pi)").unwrap() + 1.0).abs() < 1e-9);
        for bad in ["1/0", "2+", "foo(1)", "1 2", "sqrt(-1)", "(1", "", "9^999"] {
            assert!(calc::eval(bad).is_err(), "{bad}");
        }
        assert!(calc::eval(&"(".repeat(100)).is_err());
        assert!(calc::eval(&"-".repeat(100)).is_err());
    }

    #[test]
    fn unit_conversion() {
        let r = |v, f, t| {
            run("unit_convert", json!({"value": v, "from": f, "to": t})).unwrap()["result"]
                .as_f64()
                .unwrap()
        };
        assert!((r(1.0, "mi", "km") - 1.609344).abs() < 1e-9);
        assert!((r(100.0, "c", "f") - 212.0).abs() < 1e-9);
        assert!((r(1.0, "gb", "mb") - 1024.0).abs() < 1e-9);
        assert!(run(
            "unit_convert",
            json!({"value": 1, "from": "kg", "to": "km"})
        )
        .is_err());
        assert!(run("unit_convert", json!({"value": 1, "from": "c", "to": "kg"})).is_err());
    }

    #[test]
    fn date_math() {
        let r = |a: Value| run("date_calc", a).unwrap();
        assert_eq!(
            r(json!({"op": "weekday", "date": "2026-11-24"}))["weekday"],
            "Tuesday"
        );
        assert_eq!(
            r(json!({"op": "weekday", "date": "1970-01-01"}))["weekday"],
            "Thursday"
        );
        assert_eq!(
            r(json!({"op": "days_between", "date": "2024-02-01", "other_date": "2024-03-01"}))
                ["days"],
            29
        );
        assert_eq!(
            r(json!({"op": "add_days", "date": "2026-12-25", "days": 10}))["date"],
            "2027-01-04"
        );
        assert!(run("date_calc", json!({"op": "weekday", "date": "2023-02-30"})).is_err());
        assert!(run("date_calc", json!({"op": "weekday", "date": "24-1-1"})).is_err());
    }

    #[test]
    fn text_stats_counts_arabic_correctly() {
        let v = run("text_stats", json!({"text": "مرحبا بالعالم\nسطر ثاني"})).unwrap();
        assert_eq!(v["words"], 4);
        assert_eq!(v["lines"], 2);
        assert_eq!(v["characters"], 22);
    }

    #[test]
    fn summaries_are_human_readable() {
        let reg = crate::ToolRegistry::with_builtins();
        let sum = |t: &str, a: Value| summarize(reg.get(t).unwrap(), &a);
        assert_eq!(
            sum("set_alarm", json!({"hour": 6, "minute": 30})),
            "⏰ 06:30"
        );
        assert_eq!(sum("set_timer", json!({"minutes": 10})), "⏱ 10 min 0 s");
        assert_eq!(sum("flashlight", json!({"on": true})), "🔦 ON");
    }

    #[test]
    fn prechecks_block_unsafe_host_arguments() {
        assert!(precheck("open_url", &json!({"url": "https://example.com/a"})).is_ok());
        for u in [
            "http://x.com",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://",
            "https://a b",
        ] {
            assert!(precheck("open_url", &json!({"url": u})).is_err(), "{u}");
        }
        assert!(precheck("dial_number", &json!({"phone": "+201234567890"})).is_ok());
        assert!(precheck("dial_number", &json!({"phone": "*#06#"})).is_err());
        assert!(precheck("whatsapp_compose", &json!({"text": "x", "phone": "12; rm"})).is_err());
        assert!(precheck("calendar_event", &json!({"start": "2026-12-01T09:30"})).is_ok());
        assert!(precheck("calendar_event", &json!({"start": "2026-12-01 9:30"})).is_err());
        assert!(precheck("email_compose", &json!({"to": "a@b.c"})).is_ok());
        assert!(precheck("set_timer", &json!({"minutes": 10})).is_ok());
        assert!(precheck("set_timer", &json!({})).is_err());
        assert!(precheck("email_compose", &json!({"to": "nope"})).is_err());
    }
}
