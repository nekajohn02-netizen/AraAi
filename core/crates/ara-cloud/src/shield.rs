//! Privacy Shield: replaces personal data with placeholders before text
//! leaves the phone, and restores it in the streamed reply.
//!
//! Detected: e-mail addresses, phone numbers (7–15 digits, optional +),
//! payment card numbers (Luhn-valid 13–19 digits), Egyptian national IDs
//! (14 digits starting with 2 or 3), IBANs, and secret-looking tokens
//! (`sk-…`, `ghp_…`, `AKIA…`, `AIza…`). Placeholders look like `[PHONE_1]`.
//! This is best-effort pattern matching, not a guarantee; the consent screen
//! says so.

use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct Shield {
    /// placeholder -> original
    map: HashMap<String, String>,
    /// original -> placeholder (same value gets the same placeholder)
    rev: HashMap<String, String>,
    counters: HashMap<&'static str, u32>,
}

impl Shield {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn redacted_count(&self) -> usize {
        self.map.len()
    }

    fn placeholder(&mut self, kind: &'static str, original: &str) -> String {
        if let Some(p) = self.rev.get(original) {
            return p.clone();
        }
        let n = self.counters.entry(kind).or_insert(0);
        *n += 1;
        let p = format!("[{kind}_{n}]");
        self.map.insert(p.clone(), original.to_string());
        self.rev.insert(original.to_string(), p.clone());
        p
    }

    /// Redacts one text. Call for every outgoing message with the same Shield
    /// so placeholders stay consistent across the conversation.
    pub fn redact(&mut self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < chars.len() {
            if let Some((len, kind)) = detect(&chars, i) {
                let original: String = chars[i..i + len].iter().collect();
                out.push_str(&self.placeholder(kind, &original));
                i += len;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    /// Restores placeholders in a complete text.
    pub fn restore(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (p, o) in &self.map {
            out = out.replace(p, o);
        }
        out
    }

    /// Streaming restore: returns text safe to show now, holding back a
    /// possibly incomplete trailing placeholder.
    pub fn restore_stream(&self, pending: &mut String, chunk: &str) -> String {
        pending.push_str(chunk);
        let cut = match pending.rfind('[') {
            Some(i) if !pending[i..].contains(']') && pending.len() - i <= 24 => i,
            _ => pending.len(),
        };
        let ready: String = pending[..cut].to_string();
        pending.drain(..cut);
        self.restore(&ready)
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Returns (length in chars, kind) of a sensitive token starting at `i`.
fn detect(c: &[char], i: usize) -> Option<(usize, &'static str)> {
    let prev_word = i > 0 && is_word_char(c[i - 1]);
    // E-mail: local@domain.tld
    if !prev_word && is_email_local(c[i]) {
        let mut j = i;
        while j < c.len() && is_email_local(c[j]) {
            j += 1;
        }
        if j < c.len() && c[j] == '@' {
            let mut k = j + 1;
            let mut dot = false;
            while k < c.len() && (c[k].is_ascii_alphanumeric() || c[k] == '-' || c[k] == '.') {
                dot |= c[k] == '.';
                k += 1;
            }
            while k > j + 1 && c[k - 1] == '.' {
                k -= 1;
            }
            if dot && k > j + 3 {
                return Some((k - i, "EMAIL"));
            }
        }
    }
    // Secrets: well-known API key prefixes followed by a long token.
    if !prev_word {
        for prefix in ["sk-", "sk_", "ghp_", "gho_", "xoxb-", "AKIA", "AIza"] {
            let pc: Vec<char> = prefix.chars().collect();
            if c[i..].starts_with(&pc) {
                let mut j = i + pc.len();
                while j < c.len() && (c[j].is_ascii_alphanumeric() || c[j] == '-' || c[j] == '_') {
                    j += 1;
                }
                if j - i >= pc.len() + 12 {
                    return Some((j - i, "SECRET"));
                }
            }
        }
    }
    // IBAN: 2 letters, 2 digits, 11–30 alphanumerics (spaces allowed).
    if !prev_word
        && i + 4 <= c.len()
        && c[i].is_ascii_uppercase()
        && c[i + 1].is_ascii_uppercase()
        && c[i + 2].is_ascii_digit()
        && c[i + 3].is_ascii_digit()
    {
        let mut j = i + 4;
        let mut n = 0;
        while j < c.len()
            && (c[j].is_ascii_alphanumeric()
                || (c[j] == ' ' && j + 1 < c.len() && c[j + 1].is_ascii_alphanumeric()))
        {
            if c[j] != ' ' {
                n += 1;
            }
            j += 1;
        }
        if (11..=30).contains(&n) {
            return Some((j - i, "IBAN"));
        }
    }
    // Digit runs: phone / card / national id. Separators: space, -, ., ().
    let start_ok = !prev_word && (c[i] == '+' || c[i].is_ascii_digit() || is_arabic_digit(c[i]));
    if start_ok {
        let mut j = i;
        let mut digits = String::new();
        if c[j] == '+' {
            j += 1;
        }
        while j < c.len() {
            if let Some(d) = digit_value(c[j]) {
                digits.push(d);
                j += 1;
            } else if matches!(c[j], ' ' | '-' | '.' | '(' | ')')
                && j + 1 < c.len()
                && digit_value(c[j + 1]).is_some()
                && !digits.is_empty()
            {
                j += 1;
            } else {
                break;
            }
        }
        if j < c.len() && is_word_char(c[j]) {
            return None; // part of a longer word like "v2.3abc"
        }
        let n = digits.len();
        if n == 14 && (digits.starts_with('2') || digits.starts_with('3')) {
            return Some((j - i, "NATIONAL_ID"));
        }
        if (13..=19).contains(&n) && luhn(&digits) {
            return Some((j - i, "CARD"));
        }
        if (7..=15).contains(&n) && (c[i] == '+' || digits.starts_with('0')) {
            return Some((j - i, "PHONE"));
        }
    }
    None
}

fn is_email_local(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-')
}

fn is_arabic_digit(c: char) -> bool {
    ('٠'..='٩').contains(&c)
}

fn digit_value(c: char) -> Option<char> {
    if c.is_ascii_digit() {
        Some(c)
    } else if is_arabic_digit(c) {
        char::from_digit(c as u32 - '٠' as u32, 10)
    } else {
        None
    }
}

fn luhn(d: &str) -> bool {
    let mut sum = 0;
    for (i, ch) in d.chars().rev().enumerate() {
        let mut v = ch.to_digit(10).unwrap_or(0);
        if i % 2 == 1 {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
    }
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_personal_data_and_restores_it() {
        let mut s = Shield::new();
        let t = "كلم أحمد على 01001234567 أو +20 100 123 4567، إيميله ahmed.ali@example.com \
والكارت 4111 1111 1111 1111 والرقم القومي 29801011234567 ومفتاحي sk-abcdefghijklmnop1234";
        let r = s.redact(t);
        for leaked in [
            "01001234567",
            "ahmed.ali@example.com",
            "4111",
            "29801011234567",
            "sk-abcdef",
        ] {
            assert!(!r.contains(leaked), "{leaked} leaked in: {r}");
        }
        assert!(r.contains("[PHONE_1]") && r.contains("[EMAIL_1]") && r.contains("[CARD_1]"));
        assert!(r.contains("[NATIONAL_ID_1]") && r.contains("[SECRET_1]"));
        assert_eq!(s.restore(&r), t);
    }

    #[test]
    fn keeps_ordinary_numbers_and_words() {
        let mut s = Shield::new();
        let t = "السعر 2500 جنيه سنة 2026، الإصدار v2.3، والكود ABC123 والساعة 10:30";
        assert_eq!(s.redact(t), t);
        assert_eq!(s.redacted_count(), 0);
    }

    #[test]
    fn same_value_same_placeholder_and_arabic_digits() {
        let mut s = Shield::new();
        let a = s.redact("رقمي ٠١٠٠١٢٣٤٥٦٧");
        let b = s.redact("تاني: ٠١٠٠١٢٣٤٥٦٧");
        assert!(a.contains("[PHONE_1]") && b.contains("[PHONE_1]"));
        assert_eq!(s.redacted_count(), 1);
    }

    #[test]
    fn iban_and_invalid_card_handling() {
        let mut s = Shield::new();
        assert!(s
            .redact("IBAN EG380019000500000000263180002")
            .contains("[IBAN_1]"));
        // 16 digits that fail Luhn and don't start with 0/+ are left alone.
        assert_eq!(s.redact("order 1234567812345678"), "order 1234567812345678");
    }

    #[test]
    fn streaming_restore_handles_split_placeholders() {
        let mut s = Shield::new();
        s.redact("call 01001234567");
        let mut pending = String::new();
        let mut out = String::new();
        for chunk in ["I'll call [PH", "ONE_", "1] now [not a tag"] {
            out.push_str(&s.restore_stream(&mut pending, chunk));
        }
        out.push_str(&s.restore(&std::mem::take(&mut pending)));
        assert_eq!(out, "I'll call 01001234567 now [not a tag");
    }
}
