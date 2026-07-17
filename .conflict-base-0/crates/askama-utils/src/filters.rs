use std::cell::RefCell;

use isolang::Language;

thread_local! {
    static CURRENT_DATE_OVERRIDE: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn set_current_date_override(date: Option<String>) {
    CURRENT_DATE_OVERRIDE.with(|v| *v.borrow_mut() = date);
}

fn extract_iso639(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

#[askama::filter_fn]
pub fn current_date<T: ?Sized>(_value: &T, _env: &dyn askama::Values) -> askama::Result<String> {
    CURRENT_DATE_OVERRIDE.with(|v| {
        if let Some(ref date) = *v.borrow() {
            return Ok(date.clone());
        }
        Ok(chrono::Utc::now().format("%Y-%m-%d").to_string())
    })
}

#[askama::filter_fn]
pub fn language(value: &Option<String>, _env: &dyn askama::Values) -> askama::Result<String> {
    let raw = value.as_deref().unwrap_or("").to_lowercase();
    let v = extract_iso639(&raw);
    let lang = Language::from_639_1(v).unwrap_or(Language::from_639_1("en").unwrap());
    Ok(lang.to_name().to_string())
}

#[askama::filter_fn]
pub fn is_english(value: &Option<String>, _env: &dyn askama::Values) -> askama::Result<bool> {
    let raw = value.as_deref().unwrap_or("en").to_lowercase();
    let v = extract_iso639(&raw);
    let lang = Language::from_639_1(v);
    Ok(matches!(lang, Some(Language::Eng)))
}

#[askama::filter_fn]
pub fn is_korean(value: &Option<String>, _env: &dyn askama::Values) -> askama::Result<bool> {
    let raw = value.as_deref().unwrap_or("en").to_lowercase();
    let v = extract_iso639(&raw);
    let lang = Language::from_639_1(v);
    Ok(matches!(lang, Some(Language::Kor)))
}

pub const TEMPLATE_FILTERS: &[&str] = &["current_date", "language", "is_english", "is_korean"];

#[cfg(test)]
mod tests {
    mod filters {
        pub use super::super::*;
    }

    use super::*;
    use crate::tpl_assert;
    use askama::Template;

    #[test]
    fn test_isolang() {
        assert!(matches!(Language::from_639_1("en"), Some(Language::Eng)));
        assert!(matches!(Language::from_639_1("ko"), Some(Language::Kor)));

        assert!(matches!(Language::from_639_1("EN"), None));
        assert!(matches!(Language::from_639_1("KO"), None));
    }

    #[derive(Template)]
    #[template(source = "{{ lang|language }}", ext = "txt")]
    struct LanguageFilterTest {
        lang: Option<String>,
    }

    tpl_assert!(
        test_language_filter_english,
        LanguageFilterTest {
            lang: Some("en".to_string())
        },
        |v| v == "English"
    );

    tpl_assert!(
        test_language_filter_korean,
        LanguageFilterTest {
            lang: Some("ko".to_string())
        },
        |v| v == "Korean"
    );

    tpl_assert!(
        test_language_filter_uppercase_defaults_to_english,
        LanguageFilterTest {
            lang: Some("EN".to_string())
        },
        |v| v == "English"
    );

    tpl_assert!(
        test_language_filter_none_defaults_to_english,
        LanguageFilterTest { lang: None },
        |v| v == "English"
    );

    #[derive(Template)]
    #[template(
        source = "{% if lang|is_english %}yes{% else %}no{% endif %}",
        ext = "txt"
    )]
    struct IsEnglishFilterTest {
        lang: Option<String>,
    }

    tpl_assert!(
        test_is_english_filter_with_en,
        IsEnglishFilterTest {
            lang: Some("en".to_string())
        },
        |v| v == "yes"
    );

    tpl_assert!(
        test_is_english_filter_with_ko,
        IsEnglishFilterTest {
            lang: Some("ko".to_string())
        },
        |v| v == "no"
    );

    tpl_assert!(
        test_is_english_filter_none_defaults_to_english,
        IsEnglishFilterTest { lang: None },
        |v| v == "yes"
    );

    #[derive(Template)]
    #[template(
        source = "{% if lang|is_korean %}yes{% else %}no{% endif %}",
        ext = "txt"
    )]
    struct IsKoreanFilterTest {
        lang: Option<String>,
    }

    tpl_assert!(
        test_is_korean_filter_with_ko,
        IsKoreanFilterTest {
            lang: Some("ko".to_string())
        },
        |v| v == "yes"
    );

    tpl_assert!(
        test_is_korean_filter_with_en,
        IsKoreanFilterTest {
            lang: Some("en".to_string())
        },
        |v| v == "no"
    );

    tpl_assert!(
        test_is_korean_filter_none_defaults_to_english,
        IsKoreanFilterTest { lang: None },
        |v| v == "no"
    );

    tpl_assert!(
        test_language_filter_bcp47_french,
        LanguageFilterTest {
            lang: Some("fr-FR".to_string())
        },
        |v| v == "French"
    );

    tpl_assert!(
        test_language_filter_bcp47_german,
        LanguageFilterTest {
            lang: Some("de-DE".to_string())
        },
        |v| v == "German"
    );

    tpl_assert!(
        test_is_english_filter_bcp47_french,
        IsEnglishFilterTest {
            lang: Some("fr-FR".to_string())
        },
        |v| v == "no"
    );

    tpl_assert!(
        test_is_english_filter_bcp47_english,
        IsEnglishFilterTest {
            lang: Some("en-US".to_string())
        },
        |v| v == "yes"
    );

    tpl_assert!(
        test_is_korean_filter_bcp47_korean,
        IsKoreanFilterTest {
            lang: Some("ko-KR".to_string())
        },
        |v| v == "yes"
    );
}
