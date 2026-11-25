//! Cross-dialect / cross-language query expansion — a curated lookup table,
//! not a trained model.
//!
//! Rationale: fine-tuning an LLM for Egyptian Arabic needs a GPU, a labeled
//! dataset and ongoing retraining, and the result still has to fit a phone's
//! RAM. Retrieval doesn't need any of that: a static crosswalk of
//! MSA/Egyptian/English equivalents (`../../../lexicon/ar_crosswalk.json`,
//! edited as data, no retraining) plus terms the *user* has explicitly
//! confirmed (`learned_aliases` in `lib.rs`) are enough to bridge "الذكاء
//! الاصطناعي التوليدي" and "Generative AI" at query time. Cost: a linear
//! scan of a few dozen short groups per query. No model is loaded for this.

use serde::Deserialize;
use std::sync::OnceLock;

use crate::normalize::normalize;

const DATA: &str = include_str!("../../../lexicon/ar_crosswalk.json");

#[derive(Deserialize)]
struct CrosswalkFile {
    #[allow(dead_code)]
    schema_version: u32,
    groups: Vec<Vec<String>>,
}

/// Groups of equivalent terms (single words or short phrases), normalized
/// once at first use.
fn groups() -> &'static Vec<Vec<String>> {
    static GROUPS: OnceLock<Vec<Vec<String>>> = OnceLock::new();
    GROUPS.get_or_init(|| {
        let file: CrosswalkFile =
            serde_json::from_str(DATA).expect("built-in ar_crosswalk.json must parse");
        file.groups
            .iter()
            .map(|g| g.iter().map(|t| normalize(t)).collect())
            .collect()
    })
}

/// For an already-normalized query, returns every term from a group that
/// has at least one member present in the query as a whole word/phrase
/// (space-bounded, so "ai" in a group never matches inside "wait"). The
/// matched term(s) are included too; callers already dedup against the
/// query's own words.
pub fn expand_query(normalized_query: &str) -> Vec<String> {
    let padded = format!(" {normalized_query} ");
    let mut out = Vec::new();
    for group in groups() {
        if group.iter().any(|t| padded.contains(&format!(" {t} "))) {
            out.extend(group.iter().cloned());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_data_has_no_term_in_two_groups() {
        let mut seen = std::collections::HashSet::new();
        for group in groups() {
            for term in group {
                assert!(seen.insert(term.clone()), "{term:?} appears in two groups");
            }
        }
    }

    #[test]
    fn expands_arabic_phrase_to_english_tech_term() {
        let out = expand_query(&normalize("الذكاء الاصطناعي التوليدي"));
        assert!(out.contains(&normalize("generative ai")));
    }

    #[test]
    fn expands_egyptian_dialect_word_to_msa() {
        let out = expand_query(&normalize("عايز أروح بكرة"));
        assert!(out.contains(&normalize("أريد")) || out.contains(&normalize("اريد")));
    }

    #[test]
    fn does_not_match_a_word_inside_a_longer_unrelated_word() {
        // "ai" must not fire just because it's a substring of "chai".
        assert!(expand_query(&normalize("chai latte")).is_empty());
    }

    #[test]
    fn unrelated_query_expands_to_nothing() {
        assert!(expand_query(&normalize("برتقال أصفر")).is_empty());
    }
}
