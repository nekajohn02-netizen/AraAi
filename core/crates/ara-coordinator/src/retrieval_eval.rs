//! Retrieval evaluation (T1.3): Recall@K and MRR for `ara-search::search`
//! against a small, fixed, reproducible corpus — no model involved, so the
//! numbers depend only on `ara-search`'s own logic (FTS + crosswalk).
//! Dataset: `core/benchmarks/retrieval_eval.json`.
//!
//! Honest limits: only 6 documents, so Recall@10 barely discriminates
//! (most queries can surface most documents through loose OR matching);
//! MRR is the more informative number here. This is a reproducible
//! regression check, not a claim about real-world retrieval quality.

use ara_core_api::AraResult;
use ara_search::SearchIndex;
use serde::{Deserialize, Serialize};

const DATA: &str = include_str!("../../../benchmarks/retrieval_eval.json");
const SEARCH_LIMIT: u32 = 10;

#[derive(Deserialize)]
struct EvalDoc {
    name: String,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Question {
    pub id: u32,
    pub query: String,
    pub expected_document: Option<String>,
    pub expected_line: Option<u32>,
    #[serde(default)]
    pub crosswalk_required: bool,
    #[serde(default)]
    pub note: String,
}

#[derive(Deserialize)]
struct File {
    documents: Vec<EvalDoc>,
    questions: Vec<Question>,
}

fn dataset() -> File {
    serde_json::from_str(DATA).expect("embedded retrieval_eval.json is valid")
}

pub fn questions() -> Vec<Question> {
    dataset().questions
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrievalReport {
    pub positive_total: usize,
    pub recall_at_5: f64,
    pub recall_at_10: f64,
    pub mrr: f64,
    pub crosswalk_required_total: usize,
    pub crosswalk_required_recall_at_10: f64,
    pub negative_total: usize,
    pub negative_correct: usize,
    pub misses: Vec<String>,
}

/// Runs every question in the dataset against a freshly built in-memory
/// index. Deterministic: same dataset in, same report out, every time.
pub fn run() -> AraResult<RetrievalReport> {
    let data = dataset();
    let mut ix = SearchIndex::open_in_memory()?;
    for d in &data.documents {
        ix.add_document(&d.name, d.text.as_bytes())?;
    }

    let mut rep = RetrievalReport::default();
    let (mut hit5, mut hit10, mut rr_sum) = (0usize, 0usize, 0.0f64);
    let mut positives = 0usize;
    let mut cw_total = 0usize;
    let mut cw_hit10 = 0usize;

    for q in &data.questions {
        let hits = ix.search(&q.query, SEARCH_LIMIT)?;
        match (&q.expected_document, q.expected_line) {
            (None, None) => {
                rep.negative_total += 1;
                if hits.is_empty() {
                    rep.negative_correct += 1;
                } else {
                    rep.misses.push(format!(
                        "#{} (negative) expected empty, got {} hit(s), top: {}:{}",
                        q.id,
                        hits.len(),
                        hits[0].document,
                        hits[0].line
                    ));
                }
            }
            (Some(doc), Some(line)) => {
                positives += 1;
                let rank = hits
                    .iter()
                    .position(|h| &h.document == doc && h.line == line)
                    .map(|i| i + 1); // 1-based
                match rank {
                    Some(r) if r <= 5 => {
                        hit5 += 1;
                        hit10 += 1;
                        rr_sum += 1.0 / r as f64;
                    }
                    Some(r) if r <= 10 => {
                        hit10 += 1;
                        rr_sum += 1.0 / r as f64;
                    }
                    _ => rep.misses.push(format!(
                        "#{} {:?} expected {doc}:{line}, got {}",
                        q.id,
                        q.query,
                        hits.first()
                            .map(|h| format!("{}:{}", h.document, h.line))
                            .unwrap_or_else(|| "no hits".into())
                    )),
                }
                if q.crosswalk_required {
                    cw_total += 1;
                    if matches!(rank, Some(r) if r <= 10) {
                        cw_hit10 += 1;
                    }
                }
            }
            (None, Some(_)) | (Some(_), None) => {
                unreachable!("dataset bug: expected_document/line must both be set or both absent")
            }
        }
    }

    rep.positive_total = positives;
    let n = positives.max(1) as f64;
    rep.recall_at_5 = hit5 as f64 / n;
    rep.recall_at_10 = hit10 as f64 / n;
    rep.mrr = rr_sum / n;
    rep.crosswalk_required_total = cw_total;
    rep.crosswalk_required_recall_at_10 = if cw_total == 0 {
        0.0
    } else {
        cw_hit10 as f64 / cw_total as f64
    };
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_is_valid() {
        let d = dataset();
        assert!(d.documents.len() >= 5);
        assert!(d.questions.len() >= 10);
        let names: std::collections::HashSet<_> = d.documents.iter().map(|x| &x.name).collect();
        for q in &d.questions {
            if let Some(doc) = &q.expected_document {
                assert!(names.contains(doc), "question {} references unknown document {doc}", q.id);
            }
            assert_eq!(
                q.expected_document.is_some(),
                q.expected_line.is_some(),
                "question {}: expected_document/expected_line must both be set or both absent",
                q.id
            );
        }
    }

    #[test]
    fn perfect_on_its_own_fixed_corpus() {
        // This is a regression check on ara-search against a dataset built
        // specifically so every positive question is answerable — not a
        // claim that real-world retrieval scores this well.
        let r = run().unwrap();
        assert!(r.misses.is_empty(), "{:#?}", r.misses);
        assert_eq!(r.recall_at_5, 1.0);
        assert_eq!(r.recall_at_10, 1.0);
        assert_eq!(r.negative_correct, r.negative_total);
        assert!(r.crosswalk_required_total >= 4, "{}", r.crosswalk_required_total);
        assert_eq!(r.crosswalk_required_recall_at_10, 1.0);
    }

    #[test]
    fn crosswalk_questions_actually_need_the_crosswalk() {
        // Sanity check on the dataset itself: a question marked
        // crosswalk_required must have zero literal word overlap with its
        // target line, or it isn't actually testing what it claims to.
        let d = dataset();
        for q in d.questions.iter().filter(|q| q.crosswalk_required) {
            let doc = d.documents.iter().find(|x| Some(&x.name) == q.expected_document.as_ref()).unwrap();
            let line = doc.text.lines().nth(q.expected_line.unwrap() as usize - 1).unwrap();
            let line_norm = ara_search::normalize::normalize(line);
            for w in ara_search::normalize::normalize(&q.query).split_whitespace() {
                assert!(
                    !line_norm.contains(w),
                    "question {} claims crosswalk_required but query word {w:?} literally appears in the target line",
                    q.id
                );
            }
        }
    }
}
