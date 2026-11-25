//! Text extraction for supported formats.

use ara_core_api::{AraError, AraResult};

const TEXT_EXT: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "log", "json", "html", "htm",
];

/// Extracted text plus, for paginated formats (PDF), where each page starts.
pub struct Extracted {
    pub text: String,
    /// 1-based line numbers (within `text`) where each page begins. Empty
    /// for formats with no page concept (everything but PDF today).
    pub page_starts: Vec<u32>,
}

/// Page number (1-based) a given 1-based `line` falls on, from a document's
/// `page_starts`. `None` if the document has no page concept.
pub fn page_for_line(page_starts: &[u32], line: u32) -> Option<u32> {
    if page_starts.is_empty() {
        return None;
    }
    // page_starts is built strictly increasing starting at 1 (see
    // extract_pdf), so the count of starts at or before `line` is the page.
    Some(page_starts.partition_point(|&s| s <= line).max(1) as u32)
}

pub fn extract_text(name: &str, bytes: &[u8]) -> AraResult<Extracted> {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if ext == "pdf" {
        return extract_pdf(bytes);
    }
    if !TEXT_EXT.contains(&ext.as_str()) {
        return Err(AraError::Unsupported(format!(
            "file type '.{ext}' is not supported yet (supported: {}, pdf)",
            TEXT_EXT.join(", ")
        )));
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes); // UTF-8 BOM
    let text = std::str::from_utf8(bytes)
        .map_err(|_| AraError::InvalidInput("file is not valid UTF-8 text".into()))?;
    if text.contains('\0') {
        return Err(AraError::InvalidInput("file looks binary".into()));
    }
    let text = match ext.as_str() {
        "json" => json_text(text)?,
        "html" | "htm" => strip_tags(text),
        _ => text.replace("\r\n", "\n"),
    };
    Ok(Extracted {
        text,
        page_starts: vec![],
    })
}

/// Extracts text page-by-page (`pdf_extract`, pure Rust, no external
/// binary or OCR). Scanned/image-only PDFs yield no text and are rejected
/// explicitly rather than returned empty — OCR is not implemented yet.
fn extract_pdf(bytes: &[u8]) -> AraResult<Extracted> {
    let pages = pdf_extract::extract_text_from_mem_by_pages(bytes)
        .map_err(|e| AraError::InvalidInput(format!("could not read pdf: {e}")))?;
    let mut text = String::new();
    let mut page_starts = Vec::with_capacity(pages.len());
    let mut line: u32 = 1;
    for page in &pages {
        page_starts.push(line);
        let page = page.replace("\r\n", "\n");
        let advance = page.chars().filter(|&c| c == '\n').count() as u32
            + u32::from(!page.is_empty() && !page.ends_with('\n'));
        text.push_str(&page);
        if !page.is_empty() && !page.ends_with('\n') {
            text.push('\n');
        }
        line += advance.max(1); // every page, even blank, occupies a line
    }
    if text.trim().is_empty() {
        return Err(AraError::InvalidInput(
            "pdf has no extractable text (likely scanned images; OCR is not implemented yet)"
                .into(),
        ));
    }
    Ok(Extracted { text, page_starts })
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

/// Builds a minimal real PDF (via `lopdf`) with one page of literal text
/// per entry in `pages`, for exercising the real parser in tests — not a
/// hand-rolled byte string standing in for a PDF. Used from this crate's
/// other test modules too (e.g. `lib.rs`'s end-to-end indexing tests).
#[cfg(test)]
pub(crate) fn make_pdf(pages: &[&str]) -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    let mut doc = Document::with_version("1.5");
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for text in pages {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![10.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(*text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
        });
        kids.push(page_id.into());
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => pages.len() as i64,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_supported_formats() {
        assert_eq!(
            extract_text("a.TXT", b"\xEF\xBB\xBFhi\r\nthere").unwrap().text,
            "hi\nthere"
        );
        let j = extract_text("d.json", r#"{"name":"علي","n":[1,2]}"#.as_bytes())
            .unwrap()
            .text;
        assert!(j.contains("علي") && j.lines().count() > 3);
        let h = extract_text(
            "p.html",
            b"<html><style>x{}</style><script>alert(1)</script><p>Hello &amp; bye</p><br>end</html>",
        )
        .unwrap()
        .text;
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

    #[test]
    fn extracts_pdf_text_and_tracks_real_page_boundaries() {
        let pdf = make_pdf(&["Page one text", "Page two text", "Page three text"]);
        let e = extract_text("doc.PDF", &pdf).unwrap();
        assert!(e.text.contains("Page one text"));
        assert!(e.text.contains("Page two text"));
        assert!(e.text.contains("Page three text"));
        assert_eq!(e.page_starts.len(), 3);
        assert_eq!(e.page_starts[0], 1, "first page always starts at line 1");
        assert!(e.page_starts[1] > e.page_starts[0]);
        assert!(e.page_starts[2] > e.page_starts[1]);

        // The line each page's text actually lands on must resolve back to
        // that same page number — the point of tracking page_starts at all.
        for (i, text) in ["Page one text", "Page two text", "Page three text"]
            .iter()
            .enumerate()
        {
            let line = e.text.lines().position(|l| l.contains(text)).unwrap() as u32 + 1;
            assert_eq!(page_for_line(&e.page_starts, line), Some(i as u32 + 1));
        }
    }

    #[test]
    fn rejects_pdf_with_no_extractable_text() {
        // A PDF whose only page has no content stream at all (e.g. a
        // scanned image with no OCR layer) must fail explicitly, not
        // silently index an empty document.
        let empty = make_pdf(&[""]);
        assert!(extract_text("scan.pdf", &empty).is_err());
    }

    #[test]
    fn page_for_line_has_no_pages_outside_range() {
        assert_eq!(page_for_line(&[], 5), None);
        assert_eq!(page_for_line(&[1, 10, 20], 1), Some(1));
        assert_eq!(page_for_line(&[1, 10, 20], 9), Some(1));
        assert_eq!(page_for_line(&[1, 10, 20], 10), Some(2));
        assert_eq!(page_for_line(&[1, 10, 20], 1000), Some(3));
    }
}
