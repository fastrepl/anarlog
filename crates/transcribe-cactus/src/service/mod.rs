mod batch;
mod streaming;
pub use streaming::*;

use std::path::Path;

use owhisper_interface::stream::{Extra, Metadata, ModelInfo};

pub(crate) fn build_metadata(model_path: &Path) -> Metadata {
    let model_name = model_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("cactus")
        .to_string();

    Metadata {
        model_info: ModelInfo {
            name: model_name,
            version: "1.0".to_string(),
            arch: "cactus".to_string(),
        },
        extra: Some(Extra::default().into()),
        ..Default::default()
    }
}

pub(crate) fn build_model(
    model_path: &Path,
    keywords: &[String],
) -> Result<hypr_cactus::Model, hypr_cactus::Error> {
    let (custom_vocabulary, vocabulary_boost) = deepgram_keywords_to_cactus_vocabulary(keywords);

    let mut model_builder = hypr_cactus::Model::builder(model_path);
    if !custom_vocabulary.is_empty() {
        model_builder = model_builder.custom_vocabulary(custom_vocabulary);
    }
    if let Some(vocabulary_boost) = vocabulary_boost {
        model_builder = model_builder.vocabulary_boost(vocabulary_boost);
    }

    model_builder.build()
}

pub(crate) fn deepgram_keywords_to_cactus_vocabulary(
    keywords: &[String],
) -> (Vec<String>, Option<f32>) {
    let mut custom_vocabulary = Vec::new();
    let mut vocabulary_boost = None;

    for keyword in keywords {
        let keyword = keyword.trim();
        if keyword.is_empty() {
            continue;
        }

        let parsed = keyword.rsplit_once(':').and_then(|(term, intensifier)| {
            let term = term.trim();
            let intensifier = intensifier.trim().parse::<f32>().ok()?;
            (!term.is_empty()).then_some((term, intensifier))
        });

        match parsed {
            Some((term, intensifier)) if intensifier > 0.0 => {
                custom_vocabulary.push(term.to_string());
                vocabulary_boost = Some(
                    vocabulary_boost.map_or(intensifier, |current: f32| current.max(intensifier)),
                );
            }
            Some(_) => {}
            None => {
                custom_vocabulary.push(keyword.to_string());
                vocabulary_boost = Some(vocabulary_boost.map_or(1.0, |current: f32| current));
            }
        }
    }

    (custom_vocabulary, vocabulary_boost)
}

#[cfg(test)]
mod tests {
    use super::deepgram_keywords_to_cactus_vocabulary;

    #[test]
    fn keeps_plain_keywords_as_vocabulary() {
        let (vocabulary, boost) = deepgram_keywords_to_cactus_vocabulary(&[
            "Hyprnote".to_string(),
            "project atlas".to_string(),
        ]);

        assert_eq!(vocabulary, vec!["Hyprnote", "project atlas"]);
        assert_eq!(boost, Some(1.0));
    }

    #[test]
    fn uses_strongest_positive_intensifier() {
        let (vocabulary, boost) = deepgram_keywords_to_cactus_vocabulary(&[
            "Hyprnote:1.5".to_string(),
            "cactus:3".to_string(),
        ]);

        assert_eq!(vocabulary, vec!["Hyprnote", "cactus"]);
        assert_eq!(boost, Some(3.0));
    }

    #[test]
    fn drops_non_positive_intensifiers() {
        let (vocabulary, boost) = deepgram_keywords_to_cactus_vocabulary(&[
            "ignore-me:0".to_string(),
            "suppress-me:-10".to_string(),
            "keep-me".to_string(),
        ]);

        assert_eq!(vocabulary, vec!["keep-me"]);
        assert_eq!(boost, Some(1.0));
    }

    #[test]
    fn keeps_colons_when_suffix_is_not_a_number() {
        let (vocabulary, boost) =
            deepgram_keywords_to_cactus_vocabulary(&["namespace:term".to_string()]);

        assert_eq!(vocabulary, vec!["namespace:term"]);
        assert_eq!(boost, Some(1.0));
    }
}
