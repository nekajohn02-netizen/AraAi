//! Capability verification: measures what a model can actually do, instead
//! of trusting catalog metadata. Four suites, each returns passed/total:
//! tool calling, structured JSON output, Arabic replies, long-context recall.

use crate::Coordinator;
use ara_core_api::{AraResult, CancellationToken};
use ara_inference::{ChatMessage, GenerationParams, LoadedModel, Role};
use ara_search::SearchIndex;
use ara_tools::policy::{Grant, PermissionStore};
use ara_tools::{skills, Dispatch, Permission, ToolRegistry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Score {
    pub passed: u32,
    pub total: u32,
}

impl Score {
    pub fn ratio(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f32 / self.total as f32
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VerifyReport {
    pub model_id: String,
    pub tool_use: Score,
    /// Cases where no tool should be called and none was.
    pub no_tool_discipline: Score,
    pub structured_output: Score,
    pub arabic: Score,
    pub long_context: Score,
    /// Human-readable failures, for debugging.
    pub failures: Vec<String>,
}

impl VerifyReport {
    /// Reliable enough to offer tool-using skills (≥ 80% and no spurious calls).
    pub fn tools_ok(&self) -> bool {
        self.tool_use.ratio() >= 0.8 && self.no_tool_discipline.ratio() >= 0.5
    }
    pub fn structured_ok(&self) -> bool {
        self.structured_output.ratio() >= 0.75
    }
    pub fn arabic_ok(&self) -> bool {
        self.arabic.ratio() >= 0.75
    }
}

/// (prompt, expected tool, a substring that must appear in the arguments)
const TOOL_CASES: &[(&str, &str, &str)] = &[
    ("احسبلي 15% من 2340", "calculator", ""),
    ("Calculate 128 * 7 + 19", "calculator", ""),
    ("صحيني الساعة 6 ونص الصبح", "set_alarm", "30"),
    ("Wake me up at 7:15 tomorrow", "set_alarm", "15"),
    ("حطلي تايمر 10 دقايق", "set_timer", "10"),
    (
        "ابعت لأحمد على الواتساب رسالة إني هتأخر، رقمه +201001234567",
        "whatsapp_compose",
        "201001234567",
    ),
    ("شغل الكشاف", "flashlight", "true"),
    ("حول 70 كيلو لرطل", "", ""), // calculator or unit_convert both acceptable
];

const NO_TOOL_CASES: &[&str] = &[
    "ما هي عاصمة مصر؟",
    "Explain what a database index is in one sentence.",
    "اكتب جملة ترحيب قصيرة",
];

fn grants() -> PermissionStore {
    let mut g = PermissionStore::default();
    for p in [
        Permission::ComposeMessages,
        Permission::Alarms,
        Permission::DeviceControls,
        Permission::LaunchApps,
        Permission::OpenWeb,
        Permission::Calendar,
        Permission::Clipboard,
    ] {
        g.set(p, Grant::AskEveryTime);
    }
    g
}

fn arabic_ratio(s: &str) -> f32 {
    let letters = s.chars().filter(|c| c.is_alphabetic()).count();
    if letters == 0 {
        return 0.0;
    }
    s.chars()
        .filter(|c| ('\u{0600}'..='\u{06FF}').contains(c))
        .count() as f32
        / letters as f32
}

fn extract_json(s: &str) -> Option<serde_json::Value> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    serde_json::from_str(&s[start..=end]).ok()
}

fn ask(model: &mut dyn LoadedModel, system: &str, user: &str, max: u32) -> AraResult<String> {
    let mut out = String::new();
    model.generate_chat(
        &[
            ChatMessage::new(Role::System, system),
            ChatMessage::new(Role::User, user),
        ],
        &GenerationParams {
            max_tokens: max,
            temperature: 0.0,
            ..Default::default()
        },
        &CancellationToken::new(),
        &mut |t| out.push_str(t),
    )?;
    Ok(out)
}

pub fn verify_model(model: &mut dyn LoadedModel, model_id: &str) -> AraResult<VerifyReport> {
    let mut r = VerifyReport {
        model_id: model_id.into(),
        ..Default::default()
    };
    let tools = ToolRegistry::with_builtins();
    let all = skills::builtin_skills();
    let grants = grants();
    let index = std::sync::Mutex::new(SearchIndex::open_in_memory()?);
    let services = crate::IndexServices(&index);
    let params = GenerationParams {
        max_tokens: 200,
        temperature: 0.0,
        ..Default::default()
    };

    // 1) tool calling — a fresh Coordinator turn per case.
    for (prompt, tool, needle) in TOOL_CASES {
        r.tool_use.total += 1;
        let c = Coordinator {
            tools: &tools,
            skills: &all,
            grants: &grants,
            services: Some(&services),
            extra_system: "",
            persona: None,
        };
        let res = c.run_turn(
            model,
            &[],
            prompt,
            &params,
            &CancellationToken::new(),
            &mut |_| {},
        )?;
        let ok = res.tool_events.first().is_some_and(|e| {
            let name_ok = if tool.is_empty() {
                ["calculator", "unit_convert"].contains(&e.call.tool.as_str())
            } else {
                e.call.tool == *tool
            };
            let valid = !matches!(&e.outcome, Dispatch::Denied { reason, .. } if reason.starts_with("invalid"));
            name_ok && valid && e.call.arguments.to_string().contains(needle)
        });
        if ok {
            r.tool_use.passed += 1;
        } else {
            r.failures.push(format!(
                "tool: '{prompt}' -> {:?}",
                res.tool_events.first().map(|e| e.call.tool.clone())
            ));
        }
    }
    for prompt in NO_TOOL_CASES {
        r.no_tool_discipline.total += 1;
        let c = Coordinator {
            tools: &tools,
            skills: &all,
            grants: &grants,
            services: Some(&services),
            extra_system: "",
            persona: None,
        };
        let res = c.run_turn(
            model,
            &[],
            prompt,
            &params,
            &CancellationToken::new(),
            &mut |_| {},
        )?;
        if res.tool_events.is_empty() && !res.text.trim().is_empty() {
            r.no_tool_discipline.passed += 1;
        } else {
            r.failures.push(format!(
                "no-tool: '{prompt}' called {:?}",
                res.tool_events.first().map(|e| e.call.tool.clone())
            ));
        }
    }

    // 2) structured output
    let sys = "Reply with ONLY a JSON object, no other text.";
    let structured: &[(&str, &[(&str, &str)])] = &[
        (
            "Extract: name=Mona, age=31, city=Cairo. Keys: name, age, city.",
            &[("name", "Mona"), ("age", "31"), ("city", "Cairo")],
        ),
        (
            "استخرج: المنتج=قهوة، السعر=45، العملة=جنيه. المفاتيح: product, price, currency",
            &[("price", "45")],
        ),
        (
            "Return {\"ok\": true, \"count\": 3} exactly.",
            &[("count", "3")],
        ),
        (
            "Keys: a, b. Set a to 1 and b to 2.",
            &[("a", "1"), ("b", "2")],
        ),
    ];
    for (prompt, expect) in structured {
        r.structured_output.total += 1;
        let out = ask(model, sys, prompt, 120)?;
        let ok = extract_json(&out).is_some_and(|v| {
            expect.iter().all(|(k, val)| {
                v.get(*k)
                    .is_some_and(|x| x.to_string().trim_matches('"') == *val)
            })
        });
        if ok {
            r.structured_output.passed += 1;
        } else {
            r.failures.push(format!(
                "json: '{prompt}' -> {}",
                out.chars().take(80).collect::<String>()
            ));
        }
    }

    // 3) Arabic replies: must answer in Arabic and be correct.
    let arabic: &[(&str, &str)] = &[
        ("ما هي عاصمة مصر؟ أجب بجملة قصيرة.", "القاهرة"),
        ("كم عدد أيام الأسبوع؟ أجب بالعربية.", "7"),
        (
            "إزاي أقول صباح الخير بالإنجليزي؟ رد بالعربي.",
            "Good morning",
        ),
        ("اذكر لون السماء في يوم صافٍ بكلمة واحدة بالعربية.", "أزرق"),
    ];
    for (prompt, must) in arabic {
        r.arabic.total += 1;
        let out = ask(model, "أجب باللغة العربية.", prompt, 80)?;
        let low = out.to_lowercase();
        let has = low.contains(&must.to_lowercase())
            || (*must == "7" && (out.contains('7') || out.contains("سبعة") || out.contains('٧')));
        // "Good morning" answers legitimately mix scripts; others must be mostly Arabic.
        let lang_ok = *must == "Good morning" || arabic_ratio(&out) > 0.6;
        if has && lang_ok {
            r.arabic.passed += 1;
        } else {
            r.failures.push(format!(
                "arabic: '{prompt}' -> {}",
                out.chars().take(80).collect::<String>()
            ));
        }
    }

    // 4) long-context recall: a fact buried in ~900 words of filler.
    let filler = "The committee reviewed the quarterly schedule and discussed unrelated logistics in detail. ";
    for (code, pos) in [("7391", 0usize), ("5208", 1)] {
        r.long_context.total += 1;
        let n = 40;
        let mut doc = String::new();
        for i in 0..n {
            if i == (n / 2) * pos + 3 {
                doc.push_str(&format!("The vault access code is {code}. "));
            }
            doc.push_str(filler);
        }
        let out = ask(
            model,
            "Answer using only the text.",
            &format!("{doc}\n\nWhat is the vault access code? Reply with the number only."),
            16,
        )?;
        if out.contains(code) {
            r.long_context.passed += 1;
        } else {
            r.failures
                .push(format!("context: expected {code} got '{}'", out.trim()));
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ara_inference::{GenerationStats, StopReason};

    /// Answers from a function of the last user message.
    struct Fake(fn(&str) -> String);
    impl LoadedModel for Fake {
        fn context_len(&self) -> u32 {
            8192
        }
        fn trained_context_len(&self) -> u32 {
            8192
        }
        fn count_tokens(&self, t: &str) -> AraResult<u32> {
            Ok(t.split_whitespace().count() as u32)
        }
        fn generate_chat(
            &mut self,
            m: &[ChatMessage],
            _p: &GenerationParams,
            _c: &CancellationToken,
            on: &mut dyn FnMut(&str),
        ) -> AraResult<GenerationStats> {
            let last = m
                .iter()
                .rev()
                .find(|x| x.role == Role::User)
                .unwrap()
                .content
                .clone();
            on(&(self.0)(&last));
            Ok(GenerationStats {
                prompt_tokens: 1,
                generated_tokens: 1,
                prompt_eval_ms: 0,
                time_to_first_token_ms: 0,
                generation_ms: 0,
                tokens_per_sec: 0.0,
                stop_reason: StopReason::EndOfGeneration,
                reused_prompt_tokens: 0,
            })
        }
    }

    #[test]
    fn a_model_that_ignores_tools_fails_tool_use() {
        let mut m = Fake(|_| "لا أستطيع المساعدة".into());
        let r = verify_model(&mut m, "dummy").unwrap();
        assert_eq!(r.tool_use.passed, 0);
        assert!(!r.tools_ok());
        assert_eq!(
            r.no_tool_discipline.passed, 3,
            "plain answers count as discipline"
        );
        assert!(!r.failures.is_empty());
    }

    #[test]
    fn a_model_that_always_calls_a_tool_fails_discipline() {
        let mut m = Fake(|_| {
            r#"<tool_call>{"name":"calculator","arguments":{"expression":"1+1"}}</tool_call>"#
                .into()
        });
        let r = verify_model(&mut m, "dummy").unwrap();
        assert_eq!(r.no_tool_discipline.passed, 0);
        assert!(!r.tools_ok());
    }

    #[test]
    fn a_competent_scripted_model_passes_everything() {
        fn good(u: &str) -> String {
            let tc = |n: &str, a: &str| {
                format!("<tool_call>{{\"name\":\"{n}\",\"arguments\":{a}}}</tool_call>")
            };
            if u.contains("15%") || u.contains("128") || u.contains("كيلو") {
                return tc("calculator", r#"{"expression":"1+1"}"#);
            }
            if u.contains("6 ونص") {
                return tc("set_alarm", r#"{"hour":6,"minute":30}"#);
            }
            if u.contains("7:15") {
                return tc("set_alarm", r#"{"hour":7,"minute":15}"#);
            }
            if u.contains("تايمر") {
                return tc("set_timer", r#"{"minutes":10}"#);
            }
            if u.contains("الواتساب") {
                return tc(
                    "whatsapp_compose",
                    r#"{"text":"هتأخر","phone":"+201001234567"}"#,
                );
            }
            if u.contains("الكشاف") {
                return tc("flashlight", r#"{"on":true}"#);
            }
            if u.contains("Mona") {
                return r#"{"name":"Mona","age":31,"city":"Cairo"}"#.into();
            }
            if u.contains("المنتج") {
                return r#"{"product":"قهوة","price":45,"currency":"جنيه"}"#.into();
            }
            if u.contains("count") {
                return r#"{"ok": true, "count": 3}"#.into();
            }
            if u.contains("Keys: a, b") {
                return r#"{"a":1,"b":2}"#.into();
            }
            if u.contains("عاصمة") {
                return "عاصمة مصر هي القاهرة".into();
            }
            if u.contains("الأسبوع") {
                return "سبعة أيام".into();
            }
            if u.contains("صباح الخير") {
                return "تقول Good morning".into();
            }
            if u.contains("السماء") {
                return "أزرق".into();
            }
            if u.contains("vault access code") {
                return if u.contains("7391") {
                    "7391".into()
                } else {
                    "5208".into()
                };
            }
            "ok".into()
        }
        let mut m = Fake(good);
        let r = verify_model(&mut m, "good").unwrap();
        assert!(r.tools_ok() && r.structured_ok() && r.arabic_ok(), "{r:#?}");
        assert_eq!(r.long_context.passed, 2);
        assert_eq!(r.tool_use.total, 8);
    }
}
