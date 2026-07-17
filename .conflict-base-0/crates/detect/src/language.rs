pub fn get_preferred_languages() -> Vec<hypr_language::Language> {
    use objc2_foundation::NSLocale;

    let languages = NSLocale::preferredLanguages();
    languages
        .iter()
        .filter_map(|s| locale_to_language(&s.to_string()))
        .collect()
}

fn locale_to_language(locale: &str) -> Option<hypr_language::Language> {
    locale.parse().ok()
}

pub fn get_current_locale_identifier() -> String {
    use objc2_foundation::NSLocale;

    let locale = NSLocale::currentLocale();
    locale.localeIdentifier().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_language::ISO639;

    #[test]
    fn test_locale_to_language() {
        let lang = locale_to_language("en-US").unwrap();
        assert_eq!(lang.iso639(), ISO639::En);
        assert_eq!(lang.region(), Some("US"));

        let lang = locale_to_language("ko-US").unwrap();
        assert_eq!(lang.iso639(), ISO639::Ko);
        assert_eq!(lang.region(), Some("US"));

        let lang = locale_to_language("ja_JP").unwrap();
        assert_eq!(lang.iso639(), ISO639::Ja);
        assert_eq!(lang.region(), Some("JP"));

        let lang = locale_to_language("zh-Hans-CN").unwrap();
        assert_eq!(lang.iso639(), ISO639::Zh);
        assert_eq!(lang.region(), Some("CN"));

        let lang = locale_to_language("en").unwrap();
        assert_eq!(lang.iso639(), ISO639::En);
        assert_eq!(lang.region(), None);

        assert!(locale_to_language("invalid").is_none());
        assert!(locale_to_language("xx-YY").is_none());
    }

    #[test]
    fn test_get_preferred_languages() {
        let languages = get_preferred_languages();
        println!("Preferred languages: {:?}", languages);
        assert!(!languages.is_empty());
    }

    #[test]
    fn test_get_current_locale_identifier() {
        let locale = get_current_locale_identifier();
        println!("Current locale: {}", locale);
        assert!(!locale.is_empty());
    }
}
