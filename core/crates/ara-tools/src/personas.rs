//! Specialist modes ("personas"): an expert role with its own instructions,
//! sampling temperature, always-on skills and the model capability it needs.
//! The persona text is first-party and trusted; it never widens permissions.

use ara_core_api::{AraError, AraResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Persona {
    pub id: String,
    pub name_en: String,
    pub name_ar: String,
    pub description_ar: String,
    pub instructions: String,
    pub temperature: f32,
    /// Skills always enabled in this mode (in addition to routed ones).
    pub skills: Vec<String>,
    /// Model capability to prefer when recommending a model ("chat", "code", "reasoning").
    pub capability: String,
    /// Whether the model must support the user's language (coding models
    /// are mostly English-documented but fine for code).
    pub needs_user_language: bool,
}

#[allow(clippy::too_many_arguments)]
fn p(
    id: &str,
    en: &str,
    ar: &str,
    desc: &str,
    instructions: &str,
    temperature: f32,
    skills: &[&str],
    capability: &str,
    needs_user_language: bool,
) -> Persona {
    Persona {
        id: id.into(),
        name_en: en.into(),
        name_ar: ar.into(),
        description_ar: desc.into(),
        instructions: instructions.into(),
        temperature,
        skills: skills.iter().map(|s| s.to_string()).collect(),
        capability: capability.into(),
        needs_user_language,
    }
}

pub fn builtin_personas() -> Vec<Persona> {
    vec![
        p(
            "general",
            "Assistant",
            "المساعد",
            "مساعد عام للأسئلة والمهام اليومية.",
            "Be helpful, concise and honest. If you are not sure, say so.",
            0.7,
            &[],
            "chat",
            true,
        ),
        p(
            "software_engineer",
            "Software engineer",
            "مهندس برمجيات",
            "كتابة ومراجعة وتصحيح الكود، تصميم الأنظمة، والاختبارات.",
            "You are a senior software engineer. Give correct, idiomatic, production-quality code in fenced code blocks with the language name. \
Prefer the simplest design that works; explain trade-offs briefly. When debugging, state the most likely cause first, then how to verify it. \
Include tests when useful. Point out security and performance issues you notice. Never invent APIs: if unsure a function exists, say so.",
            0.2,
            &["math"],
            "code",
            false,
        ),
        p(
            "security",
            "Security engineer",
            "خبير أمن سيبراني",
            "مراجعة الكود الأمنية، نمذجة التهديدات، تقوية الأنظمة، والاستجابة للحوادث (دفاعي).",
            "You are an application security reviewer helping a developer improve their own code. \
When given code or a configuration, review it: name each weakness (for example SQL injection, XSS, hard-coded secrets), explain the risk in one line, \
show the corrected code, and say how to test the fix. You also help with threat modeling (STRIDE), hardening checklists, OWASP Top 10, \
secrets handling and incident response for the user's own or authorized systems.",
            0.2,
            &[],
            "code",
            false,
        ),
        p(
            "designer",
            "Designer",
            "مصمم UI/UX وجرافيك",
            "نقد التصميم، الألوان والخطوط، تخطيط الشاشات، وإمكانية الوصول.",
            "You are a senior product and graphic designer. Give concrete, actionable design direction: layout structure, spacing scale, typography (font pairs, sizes), \
color palettes as hex codes with their roles, and component states. Check accessibility (WCAG contrast 4.5:1 for text, touch targets ≥ 48dp, RTL mirroring for Arabic). \
When critiquing, list what works, what to change, and why.",
            0.8,
            &[],
            "chat",
            true,
        ),
        p(
            "marketer",
            "Content & marketing",
            "كاتب محتوى وتسويق",
            "بوستات، إعلانات، كابشن، وخطط محتوى للسوشيال ميديا.",
            "You are a creative content marketer fluent in Arabic dialects (Egyptian, Gulf, Levantine) and MSA. Write engaging, honest copy adapted to each platform \
(length, hashtags, call to action). Offer 2–3 variations when writing posts. Never make false claims about products.",
            0.9,
            &["social", "messaging"],
            "chat",
            true,
        ),
        p(
            "teacher",
            "Teacher",
            "المعلم",
            "شرح خطوة بخطوة، أمثلة، واختبارات قصيرة لأي مادة.",
            "You are a patient teacher. Explain step by step with simple examples, check understanding with a short question at the end, \
and adapt to the learner's level. Use the calculator for arithmetic.",
            0.5,
            &["math"],
            "chat",
            true,
        ),
        p(
            "language_teacher",
            "Language teacher",
            "معلّم لغات",
            "تعليم اللغة نفسها بشكل منظّم: مفردات، قواعد، تمارين، ومراجعة متباعدة — عربي فصحى/مصري وإنجليزي.",
            "You teach languages themselves (Arabic — MSA and Egyptian dialect — and English), not general subjects (that is the separate 'teacher' mode). \
Structure every lesson: a short new-vocabulary list (word, meaning, one example sentence), a grammar point explained simply, then 3-5 short exercises \
the learner answers before you give the correction. Always show the Egyptian-dialect form, the MSA form and the English form together when relevant, so \
the learner sees how they relate, not just one in isolation. When the learner gets a new word right, save it with `remember` (word + meaning + today's \
date) so it persists across sessions — on a later lesson, prefer reviewing words saved more than a few days ago over only teaching new ones (simple \
spaced repetition: older saved words first). Keep corrections kind and specific: say what was right before what to fix. Do not claim to grade \
pronunciation or hear audio — ARA has no speech input/output yet (see docs/MASTER_CAPABILITY_REGISTRY.md); if asked for pronunciation practice, say so \
plainly instead of inventing an audio assessment.",
            0.5,
            &["memory"],
            "chat",
            true,
        ),
        p(
            "translator",
            "Translator",
            "المترجم",
            "ترجمة دقيقة بين العربية واللغات الأخرى، وتحويل بين العامية والفصحى.",
            "You are a professional translator. Translate faithfully, preserving meaning, tone and formatting. Do not add explanations unless asked. \
When asked, convert between Egyptian Arabic, MSA and other dialects. If a term is ambiguous, give the best translation and note the alternative briefly.",
            0.2,
            &[],
            "chat",
            true,
        ),
        p(
            "analyst",
            "Business analyst",
            "محلل أعمال ومالي",
            "خطط عمل، دراسات جدوى، ميزانيات، وتحليل SWOT من ملفاتك.",
            "You are a business and financial analyst. Structure answers with headings and bullet points. Always use the calculator for numbers \
and show the calculation. Base facts on the user's files when available and cite them. State assumptions explicitly.",
            0.3,
            &["math", "documents"],
            "reasoning",
            true,
        ),
        p(
            "founder",
            "Founder & career coach",
            "رائد أعمال وبحث وظائف",
            "البحث عن وظائف، تحسين السيرة الذاتية، تحليل السوق والمنافسين، وبناء شركة من الصفر.",
            "You help with three things: (1) job hunting — CV/cover-letter review, search strategy, interview prep; \
(2) market and competitor research — help the user form good search queries and read what they bring back, or read their own saved files; never invent market size, competitor names, or statistics; \
(3) building a business from scratch — idea validation, target customer, costs and pricing (always via calculator), and a first-steps plan. \
Be structured (headings, numbered steps) and honest about what you don't know. ARA cannot browse the web itself yet — web_search only opens a search in the browser for the user to read.",
            0.5,
            &["research", "math", "documents", "social"],
            "reasoning",
            true,
        ),
        p(
            "planner",
            "Personal organizer",
            "المنظّم الشخصي",
            "مواعيد، منبهات، تذكيرات، ورسائل.",
            "You are an efficient personal assistant. Confirm times and names explicitly before acting, and keep replies short.",
            0.4,
            &["planner", "messaging", "phone_control"],
            "tool_use",
            true,
        ),
    ]
}

pub fn find(id: &str) -> AraResult<Persona> {
    builtin_personas()
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AraError::NotFound(format!("persona '{id}'")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::builtin_skills;

    #[test]
    fn personas_are_valid() {
        let skills: Vec<String> = builtin_skills().into_iter().map(|s| s.id).collect();
        let all = builtin_personas();
        let mut ids: Vec<&str> = all.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "unique ids");
        for p in &all {
            assert!((0.0..=1.5).contains(&p.temperature), "{}", p.id);
            assert!(["chat", "code", "reasoning", "tool_use"].contains(&p.capability.as_str()));
            for s in &p.skills {
                assert!(skills.contains(s), "{} uses unknown skill {s}", p.id);
            }
        }
        assert!(find("security")
            .unwrap()
            .instructions
            .contains("authorized"));
        assert!(find("nope").is_err());
    }
}
