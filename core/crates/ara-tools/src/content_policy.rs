//! Content policy for any future media-generation tool (image/audio/video
//! synthesis). **No such tool exists in ARA today** — see
//! `docs/ROADMAP_TASKS.md` T2.6 — but the project owner asked explicitly
//! that music/song generation be blocked the same way explicit/sexual
//! content generation is, decided and tested *before* any generation tool
//! is ever wired to `dispatch()`, not after. The intended call site is the
//! same `precheck()` pattern every tool already goes through (ADR-006):
//! whichever future tool actually calls an image/audio/video model would
//! call [`check_media_prompt`] on the user-facing prompt first and refuse
//! before ever reaching the model, exactly like `open_url`'s https-only
//! check today.
//!
//! **This is a best-effort keyword check, not a classifier** — the same
//! honest limitation already documented for the Privacy Shield
//! (`docs/SECURITY_AND_PRIVACY.md`). It will miss creative rephrasings and
//! can over-match an innocent sentence that happens to contain a listed
//! word; it is a blunt backstop, not a guarantee. Normal conversation
//! *about* music (history, opinions, recommending a nasheed instead, a
//! question like "why don't you make music?") is unaffected — this module
//! is only ever meant to gate an actual generation *request* passed to a
//! generation tool, never general chat.

/// Why a media-generation prompt was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyViolation {
    /// Music/song/instrumental generation — blocked by the project
    /// owner's explicit decision, not a technical limitation.
    Music,
    /// Sexual/explicit content generation.
    Explicit,
}

impl PolicyViolation {
    /// Shown to the user verbatim (Arabic, by the project owner's explicit
    /// request 2026-11-26): a clear refusal, the religious ruling, a brief
    /// reminder of accountability, and a kind redirect — not a lecture.
    /// This exact wording is what gets refused *to the person*, so it is
    /// deliberately not a bare technical string like "not supported".
    pub fn message(self) -> &'static str {
        match self {
            PolicyViolation::Music => {
                "معلش، آرا ما تقدرش تساعدك في عمل موسيقى أو أغاني. سماع الموسيقى والغناء بهذا المعنى حرام شرعًا عند كثير من أهل العلم، وكل واحد مسؤول قدام ربنا سبحانه وتعالى يوم القيامة عن اللي يسمعه ويعمله. ابعد عنها واستبدلها بحاجة تقرّبك لربنا، وأنا موجود أساعدك في أي حاجة تانية تنفعك."
            }
            PolicyViolation::Explicit => {
                "معلش، آرا ما تقدرش تساعدك في طلب زي ده. المحتوى الجنسي/الإباحي حرام شرعًا ومعصية لله سبحانه وتعالى، وفيها حساب وعقاب يوم القيامة لمن استمر عليها بلا توبة. اتق الله وارجع لربك، وأنا موجود أساعدك في أي حاجة تانية تنفعك في دنياك وآخرتك."
            }
        }
    }
}

/// Arabic terms are matched after basic normalization (diacritics and
/// alef/yaa/taa-marbuta variants folded) so common spelling variants
/// still hit; see `normalize`. English terms are matched case-insensitively.
const MUSIC_TERMS_AR: &[&str] = &[
    "موسيقى", "موسيقي", "اغنيه", "أغنيه", "لحن", "الحان", "نغمه", "مزيكا", "كليب",
];
const MUSIC_TERMS_EN: &[&str] = &[
    "music", "song", "melody", "instrumental", "soundtrack", "beat", "jingle", "lyrics",
];

const EXPLICIT_TERMS_AR: &[&str] = &["عاري", "عاريه", "جنسي", "جنسيه", "اباحي", "اباحيه"];
const EXPLICIT_TERMS_EN: &[&str] = &[
    "porn", "pornographic", "nude", "naked", "nsfw", "explicit sex", "sexual content", "xxx",
    "erotic",
];

/// Lowercases (for the English terms) and folds the handful of Arabic
/// spelling variants that would otherwise dodge a plain substring match:
/// diacritics removed, أ/إ/آ/ٱ → ا, ى → ي, ة → ه — the same normalization
/// `ara-search::lexicon` applies for the same reason, reimplemented here in
/// miniature rather than adding a cross-crate dependency for four `replace`
/// calls (`ara-tools` has no other reason to depend on `ara-search`).
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(*c, '\u{064B}'..='\u{065F}' | '\u{0670}'))
        .map(|c| match c {
            'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
            'ى' => 'ي',
            'ة' => 'ه',
            other => other,
        })
        .collect::<String>()
        .to_lowercase()
}

fn any_hit(haystack: &str, terms: &[&str]) -> bool {
    terms.iter().any(|t| haystack.contains(t))
}

/// Checks a prompt that would be sent to a media-generation tool. `Ok(())`
/// means nothing matched — **not** a guarantee the content is acceptable,
/// only that this best-effort check found no reason to block it.
pub fn check_media_prompt(prompt: &str) -> Result<(), PolicyViolation> {
    let n = normalize(prompt);
    if any_hit(&n, EXPLICIT_TERMS_AR) || any_hit(&n, EXPLICIT_TERMS_EN) {
        return Err(PolicyViolation::Explicit);
    }
    if any_hit(&n, MUSIC_TERMS_AR) || any_hit(&n, MUSIC_TERMS_EN) {
        return Err(PolicyViolation::Music);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_music_requests_in_arabic_and_english() {
        assert_eq!(
            check_media_prompt("اعمل لي أغنية حزينة عن الفرقة"),
            Err(PolicyViolation::Music)
        );
        assert_eq!(
            check_media_prompt("compose an upbeat instrumental for a video"),
            Err(PolicyViolation::Music)
        );
        assert_eq!(
            check_media_prompt("اعملي موسيقي هادية للمونتاج"),
            Err(PolicyViolation::Music)
        );
    }

    #[test]
    fn blocks_explicit_requests() {
        assert_eq!(
            check_media_prompt("generate a nude image"),
            Err(PolicyViolation::Explicit)
        );
        assert_eq!(
            check_media_prompt("صورة شخص عاري"),
            Err(PolicyViolation::Explicit)
        );
    }

    #[test]
    fn explicit_check_wins_when_a_prompt_somehow_matches_both() {
        // Not expected in practice, but the policy must still be
        // deterministic rather than silently dropping one violation.
        assert_eq!(
            check_media_prompt("an explicit nude music video"),
            Err(PolicyViolation::Explicit)
        );
    }

    #[test]
    fn ordinary_generation_requests_and_music_conversation_are_not_blocked() {
        assert!(check_media_prompt("a watercolor painting of a cat in a garden").is_ok());
        assert!(check_media_prompt("صورة قطة في حديقة بأسلوب الألوان المائية").is_ok());
        // Talking *about* music, not asking to generate it, is a different
        // call site entirely (plain chat never reaches this function) —
        // this test only documents that the function itself doesn't try
        // to be smarter than a keyword match for a sentence that has no
        // music/explicit terms in it at all.
        assert!(check_media_prompt("a quiet reading room with warm light").is_ok());
    }

    #[test]
    fn arabic_spelling_variants_still_match() {
        // أغنية vs اغنيه (hamza form, taa marbuta vs haa) — both common in
        // real typing.
        assert_eq!(
            check_media_prompt("أغنيّة فرح"),
            Err(PolicyViolation::Music)
        );
    }

    #[test]
    fn violation_messages_are_non_empty_and_distinct() {
        assert_ne!(
            PolicyViolation::Music.message(),
            PolicyViolation::Explicit.message()
        );
    }
}
