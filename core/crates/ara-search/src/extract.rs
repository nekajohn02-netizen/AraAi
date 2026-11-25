//! Text extraction for supported formats.

use ara_core_api::{AraError, AraResult};

const TEXT_EXT: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "log", "json", "html", "htm",
];

pub fn extract_text(name: &str, bytes: &[u8]) -> AraResult<String> {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !TEXT_EXT.contains(&ext.as_str()) {
        return Err(AraError::Unsupported(format!(
            "file type '.{ext}' is not supported yet (supported: {})",
            TEXT_EXT.join(", ")
        )));
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes); // UTF-8 BOM
    let text = std::str::from_utf8(bytes)
        .map_err(|_| AraError::InvalidInput("file is not valid UTF-8 text".into()))?;
    if text.contains('\0') {
        return Err(AraError::InvalidInput("file looks binary".into()));
    }
    Ok(match ext.as_str() {
        "json" => json_text(text)?,
        "html" | "htm" => strip_tags(text),
        _ => text.replace("\r\n", "\n"),
    })
}

/// Pretty-prints JSON so keys and values land on their own lines (useful
/// line numbers); invalid JSON is rejected.
fn json_text(text: &str) -> AraResult<String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|_| AraError::InvalidInput("invalid JSON".into()))?;
    Ok(serde_json::to_string_pretty(&v).unwrap_or_default())
}

/// Removes tags, scripts and styles; keeps line structure roughly.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    let b = html.as_bytes();
    while i < b.len() {
        if b[i] == b'<' {
            // Drop <script>/<style> blocks entirely, then re-check.
            let block = ["script", "style"]
                .into_iter()
                .find(|t| lower[i + 1..].starts_with(t));
            if let Some(t) = block {
                match lower[i..].find(&format!("</{t}>")) {
                    Some(end) => i += end + t.len() + 3,
                    None => break, // unterminated: drop the rest
                }
                continue;
            }
            match html[i..].find('>') {
                Some(end) => {
                    let tag = &lower[i..i + end];
                    if ["<br", "</p", "</div", "</li", "</h", "</tr"]
                        .iter()
                        .any(|p| tag.starts_with(p))
                    {
                        out.push('\n');
                    }
                    i += end + 1;
                }
                None => break,
            }
            continue;
        }
        let ch = html[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_supported_formats() {
        assert_eq!(
            extract_text("a.TXT", b"\xEF\xBB\xBFhi\r\nthere").unwrap(),
            "hi\nthere"
        );
        let j = extract_text("d.json", r#"{"name":"علي","n":[1,2]}"#.as_bytes()).unwrap();
        assert!(j.contains("علي") && j.lines().count() > 3);
        let h = extract_text(
            "p.html",
            b"<html><style>x{}</style><script>alert(1)</script><p>Hello &amp; bye</p><br>end</html>",
        )
        .unwrap();
        assert!(h.contains("Hello & bye") && h.contains("end"));
        assert!(!h.contains("alert") && !h.contains("x{}"));
    }

    #[test]
    fn rejects_binary_invalid_and_unknown() {
        assert!(extract_text("a.txt", &[0xff, 0xfe, 0x00]).is_err());
        assert!(extract_text("a.txt", b"a\0b").is_err());
        assert!(extract_text("a.json", b"{bad").is_err());
        assert!(extract_text("a.docx", b"PK").is_err());
        assert!(extract_text("noext", b"x").is_err());
    }
}
