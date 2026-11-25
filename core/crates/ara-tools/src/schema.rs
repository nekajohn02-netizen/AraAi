//! Minimal, strict argument schema (a safe subset of JSON Schema).
//! Unknown properties are rejected; strings have hard length caps.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParamKind {
    String {
        max_len: u32,
        /// If set, the value must be one of these.
        #[serde(default)]
        one_of: Vec<String>,
    },
    Integer {
        min: i64,
        max: i64,
    },
    Number {
        min: f64,
        max: f64,
    },
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamSpec {
    pub kind: ParamKind,
    pub required: bool,
    pub description: String,
}

impl ParamSpec {
    pub fn string(max_len: u32, required: bool, description: &str) -> Self {
        Self {
            kind: ParamKind::String {
                max_len,
                one_of: vec![],
            },
            required,
            description: description.into(),
        }
    }
    pub fn choice(options: &[&str], required: bool, description: &str) -> Self {
        Self {
            kind: ParamKind::String {
                max_len: 64,
                one_of: options.iter().map(|s| s.to_string()).collect(),
            },
            required,
            description: description.into(),
        }
    }
    pub fn int(min: i64, max: i64, required: bool, description: &str) -> Self {
        Self {
            kind: ParamKind::Integer { min, max },
            required,
            description: description.into(),
        }
    }
    pub fn number(min: f64, max: f64, required: bool, description: &str) -> Self {
        Self {
            kind: ParamKind::Number { min, max },
            required,
            description: description.into(),
        }
    }
    pub fn boolean(required: bool, description: &str) -> Self {
        Self {
            kind: ParamKind::Boolean,
            required,
            description: description.into(),
        }
    }
}

/// Validates `args` and returns a normalised copy (only declared keys).
pub fn validate(params: &BTreeMap<String, ParamSpec>, args: &Value) -> Result<Value, String> {
    let empty = Map::new();
    let obj = match args {
        Value::Object(o) => o,
        Value::Null => &empty,
        _ => return Err("arguments must be an object".into()),
    };
    if let Some(k) = obj.keys().find(|k| !params.contains_key(*k)) {
        return Err(format!("unknown argument '{k}'"));
    }
    let mut out = Map::new();
    for (name, spec) in params {
        let Some(v) = obj.get(name).filter(|v| !v.is_null()) else {
            if spec.required {
                return Err(format!("missing '{name}'"));
            }
            continue;
        };
        let ok = match &spec.kind {
            ParamKind::String { max_len, one_of } => match v.as_str() {
                Some(s) if s.chars().count() > *max_len as usize => {
                    return Err(format!("'{name}' longer than {max_len}"))
                }
                Some(s) if !one_of.is_empty() && !one_of.iter().any(|o| o == s) => {
                    return Err(format!("'{name}' must be one of {one_of:?}"))
                }
                Some(s) if s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') => {
                    return Err(format!("'{name}' contains control characters"))
                }
                Some(_) => true,
                None => false,
            },
            ParamKind::Integer { min, max } => match v.as_i64() {
                Some(i) if i < *min || i > *max => {
                    return Err(format!("'{name}' out of range {min}..={max}"))
                }
                Some(_) => true,
                None => false,
            },
            ParamKind::Number { min, max } => match v.as_f64() {
                Some(f) if !f.is_finite() || f < *min || f > *max => {
                    return Err(format!("'{name}' out of range"))
                }
                Some(_) => true,
                None => false,
            },
            ParamKind::Boolean => v.is_boolean(),
        };
        if !ok {
            return Err(format!("'{name}' has wrong type"));
        }
        out.insert(name.clone(), v.clone());
    }
    Ok(Value::Object(out))
}

/// JSON Schema view for the model prompt.
pub fn to_json_schema(params: &BTreeMap<String, ParamSpec>) -> Value {
    let mut props = Map::new();
    let mut required = vec![];
    for (name, p) in params {
        let mut s = match &p.kind {
            ParamKind::String { max_len, one_of } => {
                let mut m = serde_json::json!({"type": "string", "maxLength": max_len});
                if !one_of.is_empty() {
                    m["enum"] = serde_json::json!(one_of);
                }
                m
            }
            ParamKind::Integer { min, max } => {
                serde_json::json!({"type": "integer", "minimum": min, "maximum": max})
            }
            ParamKind::Number { min, max } => {
                serde_json::json!({"type": "number", "minimum": min, "maximum": max})
            }
            ParamKind::Boolean => serde_json::json!({"type": "boolean"}),
        };
        s["description"] = Value::String(p.description.clone());
        props.insert(name.clone(), s);
        if p.required {
            required.push(Value::String(name.clone()));
        }
    }
    serde_json::json!({"type": "object", "properties": props, "required": required})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params() -> BTreeMap<String, ParamSpec> {
        BTreeMap::from([
            ("text".to_string(), ParamSpec::string(10, true, "t")),
            ("n".to_string(), ParamSpec::int(0, 5, false, "n")),
            (
                "mode".to_string(),
                ParamSpec::choice(&["a", "b"], false, "m"),
            ),
        ])
    }

    #[test]
    fn accepts_valid_and_drops_nulls() {
        let v = validate(&params(), &json!({"text": "مرحبا", "n": 3, "mode": null})).unwrap();
        assert_eq!(v, json!({"text": "مرحبا", "n": 3}));
    }

    #[test]
    fn rejects_bad_input() {
        let p = params();
        for a in [
            json!({}),
            json!({"text": "12345678901"}),
            json!({"text": "x", "n": 9}),
            json!({"text": "x", "n": "3"}),
            json!({"text": "x", "mode": "c"}),
            json!({"text": "x", "other": 1}),
            json!({"text": "a\u{0007}b"}),
            json!([1, 2]),
        ] {
            assert!(validate(&p, &a).is_err(), "{a}");
        }
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        // 10 Arabic letters = 20 bytes but within max_len 10.
        assert!(validate(&params(), &json!({"text": "ابتثجحخدذر"})).is_ok());
    }

    #[test]
    fn json_schema_lists_required() {
        let s = to_json_schema(&params());
        assert_eq!(s["required"], json!(["text"]));
        assert_eq!(s["properties"]["mode"]["enum"], json!(["a", "b"]));
    }
}
