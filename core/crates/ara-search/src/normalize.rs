//! Text normalization applied identically to indexed text and queries.
//!
//! Arabic: removes tashkeel (harakat, shadda, sukun, superscript alef) and
//! tatweel; unifies أ إ آ ٱ → ا, ى → ي, ة → ه, ؤ → و, ئ → ي; converts
//! Arabic-Indic and Persian digits to ASCII. Everything is lower-cased.

pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = match c {
            '\u{064B}'..='\u{0652}' | '\u{0670}' | '\u{0640}' => continue,
            'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
            'ى' => 'ي',
            'ة' => 'ه',
            'ؤ' => 'و',
            'ئ' => 'ي',
            '٠'..='٩' => char::from(b'0' + (c as u32 - '٠' as u32) as u8),
            '۰'..='۹' => char::from(b'0' + (c as u32 - '۰' as u32) as u8),
            _ => c,
        };
        out.extend(mapped.to_lowercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn arabic_variants_collapse() {
        assert_eq!(normalize("أَحْمَد"), normalize("احمد"));
        assert_eq!(normalize("إسلام"), "اسلام");
        assert_eq!(normalize("مدرسة"), normalize("مدرسه"));
        assert_eq!(normalize("مستشفى"), normalize("مستشفي"));
        assert_eq!(normalize("جـــميل"), "جميل");
        assert_eq!(normalize("١٢٣ ۴"), "123 4");
        assert_eq!(normalize("Hello WORLD"), "hello world");
    }
}
