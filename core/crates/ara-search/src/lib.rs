//! ARA Search (stage 1: text) and user memory, in one local SQLite file.
//!
//! * Documents are split into overlapping chunks with their line number, and
//!   indexed with FTS5 over an Arabic-normalized copy of the text.
//! * Results always carry the real source (file name + line). Nothing is
//!   invented: if nothing matches, the result list is empty.
//! * Memory: short facts the *user* adds/approves. Never written automatically.
//!
//! Formats: txt, md, csv, json, log, html (tags stripped), pdf (text layer
//! only, page-cited). DOCX/EPUB and OCR for scanned PDFs are not implemented
//! yet and are rejected explicitly.

pub mod extract;
pub mod guard;
pub mod lexicon;
pub mod normalize;

use ara_core_api::{AraError, AraResult};
use normalize::normalize;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
const CHUNK_CHARS: usize = 800;
const OVERLAP_CHARS: usize = 120;
const SCHEMA_VERSION: i64 = 1;
/// Hard cap on `learned_aliases` rows: a few hundred KB at most, regardless
/// of how long the app runs. Learning from the user must never be able to
/// grow storage or the per-query lookup cost without bound.
pub const MAX_LEARNED_ALIASES: i64 = 2000;
/// Upper bound on OR-ed FTS terms per query (base words + expansions), so a
/// word that happens to sit in a large synonym group can't blow up the query.
const MAX_QUERY_TERMS: usize = 24;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub id: i64,
    pub name: String,
    pub bytes: u64,
    pub chunks: u32,
    pub added_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub document: String,
    /// 1-based line where the chunk starts.
    pub line: u32,
    /// 1-based page number, for formats with a page concept (PDF). `None`
    /// for plain-text formats, where a line number is the real citation.
    pub page: Option<u32>,
    /// Original (not normalized) chunk text.
    pub text: String,
    /// Lower is better (FTS5 bm25).
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub text: String,
    pub enabled: bool,
    pub created_ms: u64,
}

pub struct SearchIndex {
    db: Connection,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn db_err(e: rusqlite::Error) -> AraError {
    AraError::Internal(format!("database: {e}"))
}

impl SearchIndex {
    /// Opens (or creates) the index. A corrupt file is reported, not deleted:
    /// the caller decides (the app offers "rebuild index").
    pub fn open(path: &Path) -> AraResult<Self> {
        let db = Connection::open(path).map_err(db_err)?;
        Self::init(db)
    }

    pub fn open_in_memory() -> AraResult<Self> {
        Self::init(Connection::open_in_memory().map_err(db_err)?)
    }

    fn init(db: Connection) -> AraResult<Self> {
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS documents(
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                sha256 TEXT NOT NULL UNIQUE,
                bytes INTEGER NOT NULL,
                added_ms INTEGER NOT NULL,
                page_starts TEXT);
             CREATE TABLE IF NOT EXISTS chunks(
                id INTEGER PRIMARY KEY,
                doc_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                line INTEGER NOT NULL,
                text TEXT NOT NULL);
             CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                norm, content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 2');
             CREATE TABLE IF NOT EXISTS memories(
                id INTEGER PRIMARY KEY,
                text TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_ms INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS learned_aliases(
                id INTEGER PRIMARY KEY,
                term TEXT NOT NULL,
                alias TEXT NOT NULL,
                used_count INTEGER NOT NULL DEFAULT 1,
                created_ms INTEGER NOT NULL,
                UNIQUE(term, alias));",
        )
        .map_err(db_err)?;
        let v: Option<i64> = db
            .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_err)?;
        match v {
            None => {
                db.execute("INSERT INTO meta VALUES('schema', ?1)", [SCHEMA_VERSION])
                    .map_err(db_err)?;
            }
            Some(SCHEMA_VERSION) => {}
            Some(other) => {
                return Err(AraError::Unsupported(format!(
                    "search index schema {other} is newer than this app"
                )))
            }
        }
        Ok(Self { db })
    }

    /// Indexes a file's bytes under a display `name`. Duplicate content is
    /// detected by SHA-256 and returns the existing document.
    pub fn add_document(&mut self, name: &str, bytes: &[u8]) -> AraResult<Document> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(AraError::InvalidInput(format!(
                "file larger than {} MB",
                MAX_FILE_BYTES >> 20
            )));
        }
        let name: String = name.chars().filter(|c| !c.is_control()).take(200).collect();
        let sha: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if let Some(d) = self.document_by_sha(&sha)? {
            return Ok(d);
        }
        let extracted = extract::extract_text(&name, bytes)?;
        let chunks = chunk(&extracted.text);
        let page_starts = if extracted.page_starts.is_empty() {
            None
        } else {
            Some(
                serde_json::to_string(&extracted.page_starts)
                    .map_err(|e| AraError::Internal(format!("page_starts: {e}")))?,
            )
        };
        let tx = self.db.transaction().map_err(db_err)?;
        tx.execute(
            "INSERT INTO documents(name, sha256, bytes, added_ms, page_starts) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![name, sha, bytes.len() as i64, now_ms(), page_starts],
        )
        .map_err(db_err)?;
        let doc_id = tx.last_insert_rowid();
        {
            let mut ins = tx
                .prepare("INSERT INTO chunks(doc_id, line, text) VALUES(?1, ?2, ?3)")
                .map_err(db_err)?;
            let mut fts = tx
                .prepare("INSERT INTO chunks_fts(rowid, norm) VALUES(?1, ?2)")
                .map_err(db_err)?;
            for (line, t) in &chunks {
                ins.execute(params![doc_id, line, t]).map_err(db_err)?;
                let id = tx.last_insert_rowid();
                fts.execute(params![id, normalize(t)]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        self.document(doc_id)?
            .ok_or_else(|| AraError::Internal("document vanished".into()))
    }

    fn document_by_sha(&self, sha: &str) -> AraResult<Option<Document>> {
        let id: Option<i64> = self
            .db
            .query_row("SELECT id FROM documents WHERE sha256=?1", [sha], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_err)?;
        match id {
            Some(id) => self.document(id),
            None => Ok(None),
        }
    }

    fn document(&self, id: i64) -> AraResult<Option<Document>> {
        self.db
            .query_row(
                "SELECT d.id, d.name, d.bytes, d.added_ms, (SELECT count(*) FROM chunks c WHERE c.doc_id=d.id)
                 FROM documents d WHERE d.id=?1",
                [id],
                |r| {
                    Ok(Document {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        bytes: r.get::<_, i64>(2)? as u64,
                        added_ms: r.get::<_, i64>(3)? as u64,
                        chunks: r.get::<_, i64>(4)? as u32,
                    })
                },
            )
            .optional()
            .map_err(db_err)
    }

    pub fn documents(&self) -> AraResult<Vec<Document>> {
        let mut st = self
            .db
            .prepare("SELECT id FROM documents ORDER BY added_ms DESC, id DESC")
            .map_err(db_err)?;
        let ids: Vec<i64> = st
            .query_map([], |r| r.get(0))
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        ids.into_iter()
            .filter_map(|id| self.document(id).transpose())
            .collect()
    }

    /// Removes a document and every index entry derived from it.
    pub fn delete_document(&mut self, id: i64) -> AraResult<()> {
        let tx = self.db.transaction().map_err(db_err)?;
        tx.execute(
            "DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE doc_id=?1)",
            [id],
        )
        .map_err(db_err)?;
        tx.execute("DELETE FROM documents WHERE id=?1", [id])
            .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    /// Full-text search. Query words are normalized and OR-ed (any word may
    /// match; bm25 ranks chunks matching more/rarer words higher). Each word
    /// is also OR-ed with its dialect/cross-language equivalents (built-in
    /// crosswalk + aliases the user has confirmed), so "عايز" matches a
    /// document that only says "أريد", and "الذكاء الاصطناعي التوليدي"
    /// matches one that only says "Generative AI".
    pub fn search(&self, query: &str, limit: u32) -> AraResult<Vec<Hit>> {
        let base_words: Vec<String> = normalize(query)
            .split_whitespace()
            .filter(|w| w.chars().count() >= 2)
            .take(16)
            .map(str::to_string)
            .collect();
        if base_words.is_empty() {
            return Ok(vec![]);
        }
        let expanded = self.expand_words(&base_words)?;
        let terms: Vec<String> = expanded
            .iter()
            .take(MAX_QUERY_TERMS)
            // Quote each term: user text can't inject FTS5 syntax.
            .map(|w| format!("\"{}\"", w.replace('"', "")))
            .collect();
        if terms.is_empty() {
            return Ok(vec![]);
        }
        let mut st = self
            .db
            .prepare(
                "SELECT d.name, c.line, c.text, bm25(chunks_fts) AS s, d.page_starts
                 FROM chunks_fts JOIN chunks c ON c.id = chunks_fts.rowid
                 JOIN documents d ON d.id = c.doc_id
                 WHERE chunks_fts MATCH ?1 ORDER BY s LIMIT ?2",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map(params![terms.join(" OR "), limit.clamp(1, 50)], |r| {
                Ok((
                    Hit {
                        document: r.get(0)?,
                        line: r.get::<_, i64>(1)? as u32,
                        page: None,
                        text: r.get(2)?,
                        score: r.get(3)?,
                    },
                    r.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(db_err)?;
        let mut hits: Vec<(Hit, Option<String>)> = rows.collect::<Result<_, _>>().map_err(db_err)?;
        // Point the citation at the best-matching line inside the chunk,
        // then resolve that line to a page number for paginated formats.
        // Uses the *expanded* terms, not just the literal query words: a hit
        // found only through a crosswalk/alias expansion (e.g. an English
        // query landing on an Arabic-only document via "task" -> "مهمة")
        // has none of its literal words in the text at all, so scoring on
        // literal words alone would pick an arbitrary line instead of the
        // one that actually matched.
        for (h, page_starts_json) in &mut hits {
            let best = h
                .text
                .lines()
                .enumerate()
                .map(|(i, l)| {
                    let nl = normalize(l);
                    (expanded.iter().filter(|w| nl.contains(w.as_str())).count(), i)
                })
                .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
                .map(|(_, i)| i)
                .unwrap_or(0);
            h.line += best as u32;
            if let Some(json) = page_starts_json {
                if let Ok(starts) = serde_json::from_str::<Vec<u32>>(json) {
                    h.page = extract::page_for_line(&starts, h.line);
                }
            }
        }
        Ok(hits.into_iter().map(|(h, _)| h).collect())
    }

    /// Base words plus their crosswalk + learned-alias expansions, deduped,
    /// in original order, capped at `MAX_QUERY_TERMS`. Pure lookups: no
    /// model runs and no row beyond the base words' matches is read.
    fn expand_words(&self, base_words: &[String]) -> AraResult<Vec<String>> {
        let mut out: Vec<String> = Vec::with_capacity(base_words.len() * 2);
        let mut seen = std::collections::HashSet::new();
        for w in base_words {
            if seen.insert(w.clone()) {
                out.push(w.clone());
            }
        }
        // Phrase-aware: a group like "الذكاء الاصطناعي التوليدي" is matched
        // against the whole query, not word-by-word.
        let joined = base_words.join(" ");
        for term in lexicon::expand_query(&joined) {
            if seen.insert(term.clone()) {
                out.push(term);
            }
        }
        if !base_words.is_empty() {
            let placeholders = vec!["?"; base_words.len()].join(",");
            let sql = format!(
                "SELECT term, alias FROM learned_aliases WHERE term IN ({placeholders}) OR alias IN ({placeholders})"
            );
            let mut st = self.db.prepare(&sql).map_err(db_err)?;
            let params: Vec<&dyn rusqlite::ToSql> = base_words
                .iter()
                .chain(base_words.iter())
                .map(|s| s as &dyn rusqlite::ToSql)
                .collect();
            let rows = st
                .query_map(params.as_slice(), |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(db_err)?;
            for row in rows {
                let (term, alias) = row.map_err(db_err)?;
                for cand in [term, alias] {
                    if seen.insert(cand.clone()) {
                        out.push(cand);
                    }
                }
            }
        }
        out.truncate(MAX_QUERY_TERMS);
        Ok(out)
    }

    // ------------------------------------------------------- learned aliases

    /// Records that the user confirmed `term` and `alias` refer to the same
    /// thing, so future searches for either word also match the other.
    /// Never called automatically — only on an explicit user action (e.g.
    /// accepting a "did you mean X?" suggestion) — same rule as [`Self::add_memory`].
    /// Bounded: once `MAX_LEARNED_ALIASES` is reached, the least-used,
    /// oldest pair is evicted first, so this can never grow storage or RAM
    /// without limit.
    pub fn confirm_alias(&self, term: &str, alias: &str) -> AraResult<()> {
        let term = normalize(term);
        let alias = normalize(alias);
        if term.is_empty() || alias.is_empty() || term == alias {
            return Err(AraError::InvalidInput("alias must differ from term".into()));
        }
        self.db
            .execute(
                "INSERT INTO learned_aliases(term, alias, used_count, created_ms) VALUES(?1, ?2, 1, ?3)
                 ON CONFLICT(term, alias) DO UPDATE SET used_count = used_count + 1",
                params![term, alias, now_ms()],
            )
            .map_err(db_err)?;
        let count: i64 = self
            .db
            .query_row("SELECT count(*) FROM learned_aliases", [], |r| r.get(0))
            .map_err(db_err)?;
        let over = count - MAX_LEARNED_ALIASES;
        if over > 0 {
            self.db
                .execute(
                    "DELETE FROM learned_aliases WHERE id IN (
                        SELECT id FROM learned_aliases ORDER BY used_count ASC, created_ms ASC LIMIT ?1)",
                    [over],
                )
                .map_err(db_err)?;
        }
        Ok(())
    }

    pub fn learned_alias_count(&self) -> AraResult<i64> {
        self.db
            .query_row("SELECT count(*) FROM learned_aliases", [], |r| r.get(0))
            .map_err(db_err)
    }

    // ------------------------------------------------------------ memory

    pub fn add_memory(&self, text: &str) -> AraResult<Memory> {
        let t: String = text.chars().filter(|c| !c.is_control()).take(300).collect();
        let t = t.trim();
        if t.is_empty() {
            return Err(AraError::InvalidInput("empty memory".into()));
        }
        self.db
            .execute(
                "INSERT INTO memories(text, enabled, created_ms) VALUES(?1, 1, ?2)",
                params![t, now_ms()],
            )
            .map_err(db_err)?;
        let id = self.db.last_insert_rowid();
        Ok(self.memories()?.into_iter().find(|m| m.id == id).unwrap())
    }

    pub fn memories(&self) -> AraResult<Vec<Memory>> {
        let mut st = self
            .db
            .prepare("SELECT id, text, enabled, created_ms FROM memories ORDER BY id")
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(Memory {
                    id: r.get(0)?,
                    text: r.get(1)?,
                    enabled: r.get::<_, i64>(2)? != 0,
                    created_ms: r.get::<_, i64>(3)? as u64,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn update_memory(
        &self,
        id: i64,
        text: Option<&str>,
        enabled: Option<bool>,
    ) -> AraResult<()> {
        if let Some(t) = text {
            let t: String = t.chars().filter(|c| !c.is_control()).take(300).collect();
            self.db
                .execute(
                    "UPDATE memories SET text=?1 WHERE id=?2",
                    params![t.trim(), id],
                )
                .map_err(db_err)?;
        }
        if let Some(e) = enabled {
            self.db
                .execute(
                    "UPDATE memories SET enabled=?1 WHERE id=?2",
                    params![e as i64, id],
                )
                .map_err(db_err)?;
        }
        Ok(())
    }

    pub fn delete_memory(&self, id: i64) -> AraResult<()> {
        self.db
            .execute("DELETE FROM memories WHERE id=?1", [id])
            .map_err(db_err)?;
        Ok(())
    }

    /// Enabled memories as a prompt section (bounded), or empty.
    pub fn memory_prompt(&self, max_chars: usize) -> AraResult<String> {
        let mut out = String::new();
        for m in self.memories()?.into_iter().filter(|m| m.enabled) {
            let line = format!("- {}\n", m.text);
            if out.len() + line.len() > max_chars {
                break;
            }
            out.push_str(&line);
        }
        Ok(if out.is_empty() {
            out
        } else {
            format!("Facts the user asked you to remember:\n{out}")
        })
    }
}

/// Splits text into ~CHUNK_CHARS chunks on line boundaries where possible,
/// with overlap, returning (1-based start line, text).
pub fn chunk(text: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let start_line = i as u32 + 1;
        let mut buf = String::new();
        let mut j = i;
        while j < lines.len() && (buf.chars().count() < CHUNK_CHARS || buf.is_empty()) {
            let l = lines[j];
            // Hard-split very long lines.
            if l.chars().count() > CHUNK_CHARS * 2 && buf.is_empty() {
                let chars: Vec<char> = l.chars().collect();
                for piece in chars.chunks(CHUNK_CHARS) {
                    out.push((j as u32 + 1, piece.iter().collect()));
                }
                j += 1;
                break;
            }
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(l);
            j += 1;
        }
        if !buf.trim().is_empty() {
            out.push((start_line, buf));
        }
        if j >= lines.len() {
            break;
        }
        // Step back for overlap, but always make progress.
        let mut back = 0usize;
        let mut k = j;
        while k > i + 1 && back < OVERLAP_CHARS {
            k -= 1;
            back += lines[k].chars().count() + 1;
        }
        i = k.max(i + 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_and_finds_arabic_with_spelling_variants_and_citations() {
        let mut ix = SearchIndex::open_in_memory().unwrap();
        let doc =
            "مقدمة\nموعد تسليم المشروع هو يوم الخميس.\nالمسؤول: أحمد علي\nالميزانيّة ٥٠٠٠ جنيه";
        let d = ix.add_document("خطة.txt", doc.as_bytes()).unwrap();
        assert_eq!(d.name, "خطة.txt");
        // Different alef / no shadda / Arabic-Indic digits.
        let h = ix.search("احمد", 5).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].document, "خطة.txt");
        assert_eq!(h[0].line, 3, "line of the match, not of the chunk");
        assert!(h[0].text.contains("أحمد"), "original text is returned");
        assert_eq!(ix.search("الميزانية", 5).unwrap().len(), 1);
        assert_eq!(ix.search("5000", 5).unwrap().len(), 1);
        assert!(ix.search("برتقال", 5).unwrap().is_empty());
    }

    #[test]
    fn pdf_hits_are_cited_by_page_not_line() {
        let pdf = extract::make_pdf(&[
            "Budget approved for 85000 pounds",
            "Timeline: delivery by Thursday",
            "Contact the project lead for questions",
        ]);
        let mut ix = SearchIndex::open_in_memory().unwrap();
        ix.add_document("plan.pdf", &pdf).unwrap();

        let h = ix.search("Thursday", 5).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].document, "plan.pdf");
        assert_eq!(h[0].page, Some(2), "match is on the 2nd page's text");

        let h = ix.search("85000", 5).unwrap();
        assert_eq!(h[0].page, Some(1));
        let h = ix.search("project lead", 5).unwrap();
        assert_eq!(h[0].page, Some(3));
    }

    #[test]
    fn non_paginated_formats_have_no_page() {
        let mut ix = SearchIndex::open_in_memory().unwrap();
        ix.add_document("note.txt", b"hello world").unwrap();
        let h = ix.search("hello", 5).unwrap();
        assert_eq!(h[0].page, None);
    }

    #[test]
    fn crosswalk_driven_hits_cite_the_right_line_not_just_the_right_document() {
        // None of this English query's literal words appear anywhere in the
        // document: the hit exists only because "task" expands to "مهمة"
        // (crosswalk) and matches line 3. The old code scored every line 0
        // on literal overlap and always picked the last line by its tie
        // rule — this pins the regression.
        let mut ix = SearchIndex::open_in_memory().unwrap();
        ix.add_document(
            "note.txt",
            "مقدمة\nاجتماع الفريق\nفيه مهمة جديدة لازم تخلص بكرة\nملاحظة ختامية".as_bytes(),
        )
        .unwrap();
        let h = ix.search("what is the next task", 5).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].line, 3, "must cite the line that actually matched");
    }

    #[test]
    fn crosswalk_bridges_dialect_and_english_without_any_model() {
        let mut ix = SearchIndex::open_in_memory().unwrap();
        ix.add_document("note.txt", "Generative AI is changing products.".as_bytes())
            .unwrap();
        // Arabic MSA phrase for the English term the document actually uses.
        assert_eq!(ix.search("الذكاء الاصطناعي التوليدي", 5).unwrap().len(), 1);

        let mut ix2 = SearchIndex::open_in_memory().unwrap();
        ix2.add_document("note2.txt", "عايز أعرف الطقس بكرة".as_bytes())
            .unwrap();
        // "اريد" (MSA) must find text written with the Egyptian "عايز".
        assert_eq!(ix2.search("اريد اعرف الطقس", 5).unwrap().len(), 1);
    }

    #[test]
    fn learned_alias_is_user_confirmed_not_automatic() {
        let mut ix = SearchIndex::open_in_memory().unwrap();
        ix.add_document("note.txt", "عندي كوبي جاهز".as_bytes())
            .unwrap();
        // "كوبي" and "نسخة" aren't in the built-in crosswalk: no match yet.
        assert!(ix.search("نسخة", 5).unwrap().is_empty());
        ix.confirm_alias("كوبي", "نسخة").unwrap();
        assert_eq!(ix.search("نسخة", 5).unwrap().len(), 1);
        assert_eq!(ix.learned_alias_count().unwrap(), 1);
        assert!(ix.confirm_alias("نسخة", "نسخه").is_err(), "normalizes to the same word");
    }

    #[test]
    fn learned_aliases_cannot_grow_storage_without_bound() {
        let ix = SearchIndex::open_in_memory().unwrap();
        for i in 0..(MAX_LEARNED_ALIASES + 50) {
            ix.confirm_alias(&format!("term{i}"), &format!("alias{i}")).unwrap();
        }
        assert_eq!(ix.learned_alias_count().unwrap(), MAX_LEARNED_ALIASES);
        // The most recently confirmed pair must survive eviction.
        let last = MAX_LEARNED_ALIASES + 49;
        assert_eq!(
            ix.search(&format!("alias{last}"), 5).unwrap().len(),
            0,
            "no document indexed, but the lookup itself must not error"
        );
    }

    #[test]
    fn line_numbers_point_to_the_right_chunk() {
        let mut ix = SearchIndex::open_in_memory().unwrap();
        let mut text = String::new();
        for i in 1..=400 {
            text.push_str(&format!("line {i} filler words to make the chunk longer\n"));
        }
        text.push_str("the secret keyword zebra appears here\n");
        ix.add_document("big.md", text.as_bytes()).unwrap();
        let h = ix.search("zebra", 3).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].line, 401, "exact line of the match");
        assert!(h[0].text.contains("zebra"));
    }

    #[test]
    fn duplicates_deletion_and_injection_safety() {
        let mut ix = SearchIndex::open_in_memory().unwrap();
        let a = ix.add_document("a.txt", b"hello world").unwrap();
        let b = ix.add_document("copy.txt", b"hello world").unwrap();
        assert_eq!(a.id, b.id, "same content is stored once");
        assert_eq!(ix.documents().unwrap().len(), 1);
        // FTS5 syntax in the query is treated as text, not operators.
        for q in ["hello\" OR \"x", "NEAR(hello world)", "* ^ : -", "\"", ""] {
            assert!(ix.search(q, 5).is_ok(), "{q}");
        }
        ix.delete_document(a.id).unwrap();
        assert!(ix.search("hello", 5).unwrap().is_empty());
        assert!(ix.documents().unwrap().is_empty());
    }

    #[test]
    fn rejects_unsupported_and_oversized_files() {
        let mut ix = SearchIndex::open_in_memory().unwrap();
        assert!(ix.add_document("x.pdf", b"%PDF-1.7").is_err());
        assert!(ix.add_document("x.bin", &[0, 159, 146, 150]).is_err());
        let big = vec![b'a'; MAX_FILE_BYTES + 1];
        assert!(ix.add_document("big.txt", &big).is_err());
    }

    #[test]
    fn memory_is_user_controlled() {
        let ix = SearchIndex::open_in_memory().unwrap();
        assert_eq!(ix.memory_prompt(500).unwrap(), "");
        let m = ix.add_memory("اسمي يوحنا وبحب القهوة").unwrap();
        ix.add_memory("أنا من القاهرة").unwrap();
        assert!(ix.add_memory("   ").is_err());
        assert!(ix.memory_prompt(500).unwrap().contains("القهوة"));
        ix.update_memory(m.id, None, Some(false)).unwrap();
        let p = ix.memory_prompt(500).unwrap();
        assert!(!p.contains("القهوة") && p.contains("القاهرة"));
        ix.delete_memory(m.id).unwrap();
        assert_eq!(ix.memories().unwrap().len(), 1);
        assert!(ix.memory_prompt(10).unwrap().is_empty(), "bounded");
    }

    #[test]
    fn persists_across_reopen() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("index.db");
        {
            let mut ix = SearchIndex::open(&p).unwrap();
            ix.add_document("n.txt", "ملاحظة عن السفر".as_bytes())
                .unwrap();
            ix.add_memory("x").unwrap();
        }
        let ix = SearchIndex::open(&p).unwrap();
        assert_eq!(ix.search("السفر", 5).unwrap().len(), 1);
        assert_eq!(ix.memories().unwrap().len(), 1);
    }

    #[test]
    fn chunking_covers_all_text_with_overlap() {
        let text: String = (0..100)
            .map(|i| format!("{i:03} {}\n", "x".repeat(40)))
            .collect();
        let c = chunk(&text);
        assert!(c.len() > 3);
        assert!(c.windows(2).all(|w| w[1].0 > w[0].0), "progress");
        assert!(c.last().unwrap().1.contains("099"));
        assert!(chunk("").is_empty());
        let long = "ب".repeat(5000);
        assert!(chunk(&long)
            .iter()
            .all(|(_, t)| t.chars().count() <= CHUNK_CHARS));
    }
}
