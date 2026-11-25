//! Extracts tool calls from model output. Supports the Qwen / Hermes format:
//! `<tool_call>{"name": "...", "arguments": {...}}</tool_call>`.
//! Everything here is untrusted input; malformed calls are reported, not run.

use crate::ToolCall;
use serde_json::Value;

pub const MAX_CALLS_PER_TURN: usize = 4;

#[derive(Debug, PartialEq)]
pub struct Parsed {
    /// Text outside the tool-call tags (shown to the user).
    pub text: String,
    pub calls: Vec<ToolCall>,
    /// Number of malformed or excess calls that were dropped.
    pub rejected: usize,
}

pub fn parse_tool_calls(output: &str) -> Parsed {
    const OPEN: &str = "<tool_call>";
    const CLOSE: &str = "</tool_call>";
    let mut text = String::new();
    let mut calls = Vec::new();
    let mut rejected = 0;
    let mut rest = output;
    while let Some(start) = rest.find(OPEN) {
        text.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len()..];
        let Some(end) = after.find(CLOSE) else {
            rejected += 1; // unterminated: drop the remainder
            rest = "";
            break;
        };
        match serde_json::from_str::<Value>(after[..end].trim()) {
            Ok(Value::Object(o)) if calls.len() < MAX_CALLS_PER_TURN => {
                match (o.get("name").and_then(Value::as_str), o.get("arguments")) {
                    (Some(name), args) => {
                        let arguments = match args {
                            // Some models emit arguments as a JSON string.
                            Some(Value::String(s)) => {
                                serde_json::from_str(s).unwrap_or(Value::Null)
                            }
                            Some(v) => v.clone(),
                            None => Value::Null,
                        };
                        calls.push(ToolCall {
                            tool: name.to_string(),
                            arguments,
                        })
                    }
                    _ => rejected += 1,
                }
            }
            _ => rejected += 1,
        }
        rest = &after[end + CLOSE.len()..];
    }
    text.push_str(rest);
    Parsed {
        text: text.trim().to_string(),
        calls,
        rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_calls_and_keeps_text() {
        let p = parse_tool_calls(
            "حاضر.\n<tool_call>\n{\"name\": \"set_alarm\", \"arguments\": {\"hour\": 7, \"minute\": 0}}\n</tool_call>",
        );
        assert_eq!(p.text, "حاضر.");
        assert_eq!(p.calls[0].tool, "set_alarm");
        assert_eq!(p.calls[0].arguments, json!({"hour": 7, "minute": 0}));
        assert_eq!(p.rejected, 0);
    }

    #[test]
    fn string_encoded_arguments_are_decoded() {
        let p = parse_tool_calls(
            r#"<tool_call>{"name":"calculator","arguments":"{\"expression\":\"1+1\"}"}</tool_call>"#,
        );
        assert_eq!(p.calls[0].arguments, json!({"expression": "1+1"}));
    }

    #[test]
    fn malformed_unterminated_and_excess_calls_are_rejected() {
        let one = r#"<tool_call>{"name":"calculator","arguments":{}}</tool_call>"#;
        let many = one.repeat(MAX_CALLS_PER_TURN + 2);
        let p = parse_tool_calls(&format!(
            "{many}<tool_call>not json</tool_call><tool_call>{{"
        ));
        assert_eq!(p.calls.len(), MAX_CALLS_PER_TURN);
        assert_eq!(p.rejected, 2 + 1 + 1);
    }

    #[test]
    fn plain_text_has_no_calls() {
        let p = parse_tool_calls("just an answer");
        assert!(p.calls.is_empty());
        assert_eq!(p.text, "just an answer");
    }
}
