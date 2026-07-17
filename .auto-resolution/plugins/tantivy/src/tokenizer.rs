use hypr_language::ISO639;
use tantivy::Index;
use tantivy::tokenizer::{
    AsciiFoldingFilter, Language, LowerCaser, NgramTokenizer, RemoveLongFilter, Stemmer,
    TextAnalyzer,
};

fn to_tantivy_language(lang: &hypr_language::Language) -> Option<Language> {
    match lang.iso639() {
        ISO639::Ar => Some(Language::Arabic),
        ISO639::Da => Some(Language::Danish),
        ISO639::Nl => Some(Language::Dutch),
        ISO639::En => Some(Language::English),
        ISO639::Fi => Some(Language::Finnish),
        ISO639::Fr => Some(Language::French),
        ISO639::De => Some(Language::German),
        ISO639::El => Some(Language::Greek),
        ISO639::Hu => Some(Language::Hungarian),
        ISO639::It => Some(Language::Italian),
        ISO639::No => Some(Language::Norwegian),
        ISO639::Pt => Some(Language::Portuguese),
        ISO639::Ro => Some(Language::Romanian),
        ISO639::Ru => Some(Language::Russian),
        ISO639::Es => Some(Language::Spanish),
        ISO639::Sv => Some(Language::Swedish),
        ISO639::Ta => Some(Language::Tamil),
        ISO639::Tr => Some(Language::Turkish),
        _ => None,
    }
}

pub fn get_tokenizer_name_for_language(lang: &hypr_language::Language) -> &'static str {
    match to_tantivy_language(lang) {
        Some(Language::Arabic) => "lang_ar",
        Some(Language::Danish) => "lang_da",
        Some(Language::Dutch) => "lang_nl",
        Some(Language::English) => "lang_en",
        Some(Language::Finnish) => "lang_fi",
        Some(Language::French) => "lang_fr",
        Some(Language::German) => "lang_de",
        Some(Language::Greek) => "lang_el",
        Some(Language::Hungarian) => "lang_hu",
        Some(Language::Italian) => "lang_it",
        Some(Language::Norwegian) => "lang_no",
        Some(Language::Portuguese) => "lang_pt",
        Some(Language::Romanian) => "lang_ro",
        Some(Language::Russian) => "lang_ru",
        Some(Language::Spanish) => "lang_es",
        Some(Language::Swedish) => "lang_sv",
        Some(Language::Tamil) => "lang_ta",
        Some(Language::Turkish) => "lang_tr",
        None => "multilang",
    }
}

pub fn register_tokenizers(index: &Index) {
    let tokenizer_manager = index.tokenizers();

    let multilang_tokenizer = TextAnalyzer::builder(NgramTokenizer::new(1, 3, false).unwrap())
        .filter(RemoveLongFilter::limit(40))
        .filter(LowerCaser)
        .filter(AsciiFoldingFilter)
        .build();
    tokenizer_manager.register("multilang", multilang_tokenizer);

    let languages = [
        ("lang_ar", Language::Arabic),
        ("lang_da", Language::Danish),
        ("lang_nl", Language::Dutch),
        ("lang_en", Language::English),
        ("lang_fi", Language::Finnish),
        ("lang_fr", Language::French),
        ("lang_de", Language::German),
        ("lang_el", Language::Greek),
        ("lang_hu", Language::Hungarian),
        ("lang_it", Language::Italian),
        ("lang_no", Language::Norwegian),
        ("lang_pt", Language::Portuguese),
        ("lang_ro", Language::Romanian),
        ("lang_ru", Language::Russian),
        ("lang_es", Language::Spanish),
        ("lang_sv", Language::Swedish),
        ("lang_ta", Language::Tamil),
        ("lang_tr", Language::Turkish),
    ];

    for (name, lang) in languages {
        let tokenizer = TextAnalyzer::builder(NgramTokenizer::new(1, 3, false).unwrap())
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser)
            .filter(AsciiFoldingFilter)
            .filter(Stemmer::new(lang))
            .build();
        tokenizer_manager.register(name, tokenizer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::build_schema;

    #[test]
    fn test_get_tokenizer_name_for_supported_languages() {
        let test_cases = [
            (ISO639::Ar, "lang_ar"),
            (ISO639::Da, "lang_da"),
            (ISO639::Nl, "lang_nl"),
            (ISO639::En, "lang_en"),
            (ISO639::Fi, "lang_fi"),
            (ISO639::Fr, "lang_fr"),
            (ISO639::De, "lang_de"),
            (ISO639::El, "lang_el"),
            (ISO639::Hu, "lang_hu"),
            (ISO639::It, "lang_it"),
            (ISO639::No, "lang_no"),
            (ISO639::Pt, "lang_pt"),
            (ISO639::Ro, "lang_ro"),
            (ISO639::Ru, "lang_ru"),
            (ISO639::Es, "lang_es"),
            (ISO639::Sv, "lang_sv"),
            (ISO639::Ta, "lang_ta"),
            (ISO639::Tr, "lang_tr"),
        ];

        for (iso639, expected_tokenizer) in test_cases {
            let lang = hypr_language::Language::from(iso639);
            let tokenizer_name = get_tokenizer_name_for_language(&lang);
            assert_eq!(
                tokenizer_name, expected_tokenizer,
                "Expected tokenizer {} for {:?}, got {}",
                expected_tokenizer, iso639, tokenizer_name
            );
        }
    }

    #[test]
    fn test_get_tokenizer_name_for_unsupported_languages() {
        let unsupported = [ISO639::Zh, ISO639::Ja, ISO639::Ko, ISO639::Hi, ISO639::Vi];

        for iso639 in unsupported {
            let lang = hypr_language::Language::from(iso639);
            let tokenizer_name = get_tokenizer_name_for_language(&lang);
            assert_eq!(
                tokenizer_name, "multilang",
                "Expected multilang tokenizer for {:?}, got {}",
                iso639, tokenizer_name
            );
        }
    }

    #[test]
    fn test_register_tokenizers() {
        let schema = build_schema();
        let index = Index::create_in_ram(schema);
        register_tokenizers(&index);

        let tokenizer_manager = index.tokenizers();

        assert!(
            tokenizer_manager.get("multilang").is_some(),
            "multilang tokenizer should be registered"
        );
        assert!(
            tokenizer_manager.get("lang_en").is_some(),
            "lang_en tokenizer should be registered"
        );
        assert!(
            tokenizer_manager.get("lang_es").is_some(),
            "lang_es tokenizer should be registered"
        );
        assert!(
            tokenizer_manager.get("lang_fr").is_some(),
            "lang_fr tokenizer should be registered"
        );
        assert!(
            tokenizer_manager.get("lang_de").is_some(),
            "lang_de tokenizer should be registered"
        );
    }

    #[test]
    fn test_english_stemmer_tokenizer() {
        let schema = build_schema();
        let index = Index::create_in_ram(schema);
        register_tokenizers(&index);

        let tokenizer_manager = index.tokenizers();
        let mut tokenizer = tokenizer_manager.get("lang_en").unwrap();

        let mut stream = tokenizer.token_stream("running jumps quickly");
        let mut tokens = Vec::new();
        while let Some(token) = stream.next() {
            tokens.push(token.text.clone());
        }

        assert!(
            tokens.contains(&"run".to_string()),
            "English stemmer should stem 'running' to 'run', got {:?}",
            tokens
        );
        assert!(
            tokens.contains(&"jump".to_string()),
            "English stemmer should stem 'jumps' to 'jump', got {:?}",
            tokens
        );
        assert!(
            tokens.contains(&"quick".to_string()),
            "English stemmer should stem 'quickly' to 'quick', got {:?}",
            tokens
        );
    }
}
