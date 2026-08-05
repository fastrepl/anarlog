pub fn get_preferred_languages() -> Vec<anlg_language::Language> {
    sys_locale::get_locales()
        .filter_map(|locale| locale_to_language(&locale))
        .collect()
}

fn locale_to_language(locale: &str) -> Option<anlg_language::Language> {
    locale.parse().ok()
}

pub fn get_current_locale_identifier() -> String {
    sys_locale::get_locale().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_language::ISO639;

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
        let expected = sys_locale::get_locales()
            .filter_map(|locale| locale_to_language(&locale))
            .collect::<Vec<_>>();

        assert_eq!(get_preferred_languages(), expected);
    }

    #[test]
    fn test_get_current_locale_identifier() {
        assert_eq!(
            get_current_locale_identifier(),
            sys_locale::get_locale().unwrap_or_default()
        );
    }
}
