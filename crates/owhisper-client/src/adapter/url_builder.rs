use owhisper_interface::ListenParams;

use crate::providers::{Provider, is_meta_model};

use super::deepgram::{DeepgramAdapter, DeepgramModel};

#[derive(Default)]
pub struct QueryParamBuilder {
    params: Vec<(String, String)>,
}

impl QueryParamBuilder {
    pub fn new() -> Self {
        Self { params: Vec::new() }
    }

    pub fn add<V: ToString>(&mut self, key: &str, value: V) -> &mut Self {
        self.params.push((key.to_string(), value.to_string()));
        self
    }

    pub fn add_bool(&mut self, key: &str, value: bool) -> &mut Self {
        self.params.push((
            key.to_string(),
            if value { "true" } else { "false" }.to_string(),
        ));
        self
    }

    pub fn add_common_listen_params(&mut self, params: &ListenParams, channels: u8) -> &mut Self {
        let model = resolve_model_for_languages(
            params.model.as_deref(),
            &params.languages,
            Provider::Deepgram.default_live_model(),
        );
        self.add("model", model)
            .add("channels", channels)
            .add("sample_rate", params.sample_rate)
            .add("encoding", "linear16")
            .add_bool("diarize", true)
            .add_bool("punctuate", true)
            .add_bool("smart_format", true)
            .add_bool("numerals", true)
            .add_bool("filler_words", false)
            .add_bool("mip_opt_out", true)
    }

    pub fn apply_to(&self, url: &mut url::Url) {
        let mut query_pairs = url.query_pairs_mut();
        for (key, value) in &self.params {
            query_pairs.append_pair(key, value);
        }
    }

    #[cfg(test)]
    pub fn build(&self) -> Vec<(String, String)> {
        self.params.clone()
    }
}

pub fn resolve_model_for_languages<'a>(
    model: Option<&'a str>,
    languages: &[hypr_language::Language],
    default: &'a str,
) -> &'a str {
    match model {
        Some(m) if !m.is_empty() && !is_meta_model(m) => m,
        _ => DeepgramAdapter::find_model(languages)
            .map(|m| match m {
                DeepgramModel::Nova3General => "nova-3",
                DeepgramModel::Nova2General => "nova-2",
                DeepgramModel::Nova3Medical => "nova-3-medical",
                DeepgramModel::Nova2Specialized => "nova-2-meeting",
            })
            .unwrap_or(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_builder() {
        let builder = QueryParamBuilder::new();
        assert!(builder.build().is_empty());
    }

    #[test]
    fn test_add_string_value() {
        let mut builder = QueryParamBuilder::new();
        builder.add("model", "nova-3");
        assert_eq!(builder.build(), vec![("model".into(), "nova-3".into())]);
    }

    #[test]
    fn test_add_numeric_value() {
        let mut builder = QueryParamBuilder::new();
        builder.add("channels", 2);
        assert_eq!(builder.build(), vec![("channels".into(), "2".into())]);
    }

    #[test]
    fn test_add_bool_true() {
        let mut builder = QueryParamBuilder::new();
        builder.add_bool("diarize", true);
        assert_eq!(builder.build(), vec![("diarize".into(), "true".into())]);
    }

    #[test]
    fn test_add_bool_false() {
        let mut builder = QueryParamBuilder::new();
        builder.add_bool("diarize", false);
        assert_eq!(builder.build(), vec![("diarize".into(), "false".into())]);
    }

    #[test]
    fn test_chaining() {
        let mut builder = QueryParamBuilder::new();
        builder
            .add("model", "nova-3")
            .add("channels", 2)
            .add_bool("diarize", true);
        assert_eq!(
            builder.build(),
            vec![
                ("model".into(), "nova-3".into()),
                ("channels".into(), "2".into()),
                ("diarize".into(), "true".into()),
            ]
        );
    }

    #[test]
    fn test_apply_to_url() {
        let mut builder = QueryParamBuilder::new();
        builder.add("model", "nova-3").add_bool("diarize", true);

        let mut url: url::Url = "https://api.example.com/listen".parse().unwrap();
        builder.apply_to(&mut url);

        assert_eq!(
            url.as_str(),
            "https://api.example.com/listen?model=nova-3&diarize=true"
        );
    }

    #[test]
    fn test_add_common_listen_params() {
        let mut builder = QueryParamBuilder::new();
        let params = ListenParams {
            model: Some("nova-3".to_string()),
            sample_rate: 16000,
            ..Default::default()
        };
        builder.add_common_listen_params(&params, 2);

        let result = builder.build();
        assert!(result.iter().any(|(k, v)| k == "model" && v == "nova-3"));
        assert!(result.iter().any(|(k, v)| k == "channels" && v == "2"));
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "sample_rate" && v == "16000")
        );
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "encoding" && v == "linear16")
        );
        assert!(result.iter().any(|(k, v)| k == "diarize" && v == "true"));
        assert!(result.iter().any(|(k, v)| k == "punctuate" && v == "true"));
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "smart_format" && v == "true")
        );
        assert!(result.iter().any(|(k, v)| k == "numerals" && v == "true"));
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "filler_words" && v == "false")
        );
        assert!(
            result
                .iter()
                .any(|(k, v)| k == "mip_opt_out" && v == "true")
        );
    }

    #[test]
    fn test_add_common_listen_params_default_model() {
        let mut builder = QueryParamBuilder::new();
        let params = ListenParams::default();
        builder.add_common_listen_params(&params, 1);

        let result = builder.build();
        assert!(result.iter().any(|(k, v)| k == "model" && v == "nova-3"));
    }

    #[test]
    fn test_add_common_listen_params_with_cloud_model() {
        let mut builder = QueryParamBuilder::new();
        let params = ListenParams {
            model: Some("cloud".to_string()),
            sample_rate: 16000,
            ..Default::default()
        };
        builder.add_common_listen_params(&params, 1);

        let result = builder.build();
        assert!(
            result.iter().any(|(k, v)| k == "model" && v == "nova-3"),
            "cloud with default (en) should resolve to nova-3"
        );
    }

    #[test]
    fn test_add_common_listen_params_with_cloud_model_chinese() {
        let mut builder = QueryParamBuilder::new();
        let params = ListenParams {
            model: Some("cloud".to_string()),
            languages: vec![hypr_language::ISO639::Zh.into()],
            sample_rate: 16000,
            ..Default::default()
        };
        builder.add_common_listen_params(&params, 1);

        let result = builder.build();
        assert!(
            result.iter().any(|(k, v)| k == "model" && v == "nova-2"),
            "cloud with zh should resolve to nova-2 (nova-3 doesn't support zh)"
        );
    }
}
