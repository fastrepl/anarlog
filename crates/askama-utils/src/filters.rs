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

pub fn current_date_value() -> String {
    CURRENT_DATE_OVERRIDE.with(|v| {
        if let Some(ref date) = *v.borrow() {
            return date.clone();
        }
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    })
}

pub fn language_name(value: Option<&str>) -> String {
    let raw = value.unwrap_or("").to_lowercase();
    let v = extract_iso639(&raw);
    let lang = Language::from_639_1(v).unwrap_or(Language::from_639_1("en").unwrap());
    lang.to_name().to_string()
}

#[askama::filter_fn]
pub fn current_date<T: ?Sized>(_value: &T, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(current_date_value())
}

#[askama::filter_fn]
pub fn language(value: &Option<String>, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(language_name(value.as_deref()))
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

    use askama::Template;

    #[derive(Template)]
    #[template(
        source = "{{ lang|language }}|{% if lang|is_english %}en{% endif %}|{% if lang|is_korean %}ko{% endif %}",
        ext = "txt"
    )]
    struct LanguageFiltersTest {
        lang: Option<String>,
    }

    #[test]
    fn language_filters_normalize_locales_and_default_to_english() {
        for (lang, expected) in [
            (None, "English|en|"),
            (Some("en"), "English|en|"),
            (Some("EN"), "English|en|"),
            (Some("en-US"), "English|en|"),
            (Some("ko"), "Korean||ko"),
            (Some("ko-KR"), "Korean||ko"),
            (Some("fr-FR"), "French||"),
        ] {
            let rendered = LanguageFiltersTest {
                lang: lang.map(str::to_string),
            }
            .render()
            .unwrap();
            assert_eq!(rendered, expected, "lang: {lang:?}");
        }
    }
}
