use std::collections::BTreeMap;

use crate::query_params::{QueryParams, QueryValue};

enum ParamValue {
    Single(String),
    Multi(Vec<String>),
}

pub struct UpstreamUrlBuilder<'a> {
    base: url::Url,
    client_params: Vec<(String, ParamValue)>,
    default_params: Vec<(&'a str, &'a str)>,
}

impl<'a> UpstreamUrlBuilder<'a> {
    pub fn new(base: url::Url) -> Self {
        Self {
            base,
            client_params: Vec::new(),
            default_params: Vec::new(),
        }
    }

    #[must_use]
    pub fn client_params(mut self, params: &QueryParams) -> Self {
        self.client_params = params
            .iter()
            .map(|(k, v)| {
                let value = match v {
                    QueryValue::Single(s) => ParamValue::Single(s.clone()),
                    QueryValue::Multi(v) => ParamValue::Multi(v.clone()),
                };
                (k.clone(), value)
            })
            .collect();
        self
    }

    #[must_use]
    pub fn default_params(mut self, params: &'a [(&'a str, &'a str)]) -> Self {
        self.default_params = params.to_vec();
        self
    }

    pub fn build(self) -> url::Url {
        let mut single_params: BTreeMap<String, String> = BTreeMap::new();
        let mut multi_params: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (key, value) in &self.default_params {
            single_params.insert((*key).to_string(), (*value).to_string());
        }

        for (key, value) in self.client_params {
            match value {
                ParamValue::Single(s) => {
                    single_params.insert(key, s);
                }
                ParamValue::Multi(v) => {
                    single_params.remove(&key);
                    multi_params.insert(key, v);
                }
            }
        }

        let mut url = self.base;
        url.set_query(None);

        let has_params = !single_params.is_empty() || !multi_params.is_empty();
        if has_params {
            let mut query_pairs = url.query_pairs_mut();
            for (key, value) in single_params {
                query_pairs.append_pair(&key, &value);
            }
            for (key, values) in multi_params {
                for value in values {
                    query_pairs.append_pair(&key, &value);
                }
            }
        }

        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_params(pairs: &[(&str, &str)]) -> QueryParams {
        let mut params = QueryParams::default();
        for (k, v) in pairs {
            params.insert(k.to_string(), QueryValue::Single(v.to_string()));
        }
        params
    }

    fn make_params_multi(pairs: &[(&str, Vec<&str>)]) -> QueryParams {
        let mut params = QueryParams::default();
        for (k, values) in pairs {
            let value = if values.len() == 1 {
                QueryValue::Single(values[0].to_string())
            } else {
                QueryValue::Multi(values.iter().map(|s| s.to_string()).collect())
            };
            params.insert(k.to_string(), value);
        }
        params
    }

    fn get_query_params(url: &url::Url) -> BTreeMap<String, String> {
        url.query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    fn get_query_params_multi(url: &url::Url) -> BTreeMap<String, Vec<String>> {
        let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (k, v) in url.query_pairs() {
            result
                .entry(k.into_owned())
                .or_default()
                .push(v.into_owned());
        }
        result
    }

    #[test]
    fn test_defaults_only() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let defaults: &[(&str, &str)] = &[("model", "nova-3-general"), ("mip_opt_out", "false")];

        let url = UpstreamUrlBuilder::new(base)
            .default_params(defaults)
            .build();

        let params = get_query_params(&url);
        assert_eq!(params.get("model"), Some(&"nova-3-general".to_string()));
        assert_eq!(params.get("mip_opt_out"), Some(&"false".to_string()));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_client_params_only() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let client = make_params(&[
            ("encoding", "linear16"),
            ("sample_rate", "16000"),
            ("channels", "1"),
        ]);

        let url = UpstreamUrlBuilder::new(base).client_params(&client).build();

        let params = get_query_params(&url);
        assert_eq!(params.get("encoding"), Some(&"linear16".to_string()));
        assert_eq!(params.get("sample_rate"), Some(&"16000".to_string()));
        assert_eq!(params.get("channels"), Some(&"1".to_string()));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_client_overrides_defaults() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let defaults: &[(&str, &str)] = &[("model", "nova-3-general"), ("mip_opt_out", "false")];
        let client = make_params(&[("model", "nova-3"), ("mip_opt_out", "true")]);

        let url = UpstreamUrlBuilder::new(base)
            .default_params(defaults)
            .client_params(&client)
            .build();

        let params = get_query_params(&url);
        assert_eq!(
            params.get("model"),
            Some(&"nova-3".to_string()),
            "client model should override default"
        );
        assert_eq!(
            params.get("mip_opt_out"),
            Some(&"true".to_string()),
            "client mip_opt_out should override default"
        );
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_partial_override() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let defaults: &[(&str, &str)] = &[("model", "nova-3-general"), ("mip_opt_out", "false")];
        let client = make_params(&[("model", "nova-3"), ("encoding", "linear16")]);

        let url = UpstreamUrlBuilder::new(base)
            .default_params(defaults)
            .client_params(&client)
            .build();

        let params = get_query_params(&url);
        assert_eq!(
            params.get("model"),
            Some(&"nova-3".to_string()),
            "client model should override default"
        );
        assert_eq!(
            params.get("mip_opt_out"),
            Some(&"false".to_string()),
            "default mip_opt_out should be preserved"
        );
        assert_eq!(
            params.get("encoding"),
            Some(&"linear16".to_string()),
            "client encoding should be added"
        );
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_empty_params() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();

        let url = UpstreamUrlBuilder::new(base.clone()).build();

        assert_eq!(url.query(), None);
        assert_eq!(url.as_str(), "wss://api.deepgram.com/v1/listen");
    }

    #[test]
    fn test_base_url_query_is_cleared() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen?existing=param"
            .parse()
            .unwrap();
        let client = make_params(&[("encoding", "linear16")]);

        let url = UpstreamUrlBuilder::new(base).client_params(&client).build();

        let params = get_query_params(&url);
        assert!(
            !params.contains_key("existing"),
            "existing query params should be cleared"
        );
        assert_eq!(params.get("encoding"), Some(&"linear16".to_string()));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_deterministic_ordering() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let client = make_params(&[("z_param", "z"), ("a_param", "a"), ("m_param", "m")]);

        let url1 = UpstreamUrlBuilder::new(base.clone())
            .client_params(&client)
            .build();
        let url2 = UpstreamUrlBuilder::new(base).client_params(&client).build();

        assert_eq!(
            url1.as_str(),
            url2.as_str(),
            "URL should be deterministic regardless of HashMap iteration order"
        );

        let query = url1.query().unwrap();
        assert!(
            query.starts_with("a_param="),
            "params should be sorted alphabetically"
        );
    }

    #[test]
    fn test_deepgram_real_world_scenario() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let defaults: &[(&str, &str)] = &[("model", "nova-3-general"), ("mip_opt_out", "false")];
        let client = make_params(&[
            ("model", "nova-3"),
            ("mip_opt_out", "true"),
            ("encoding", "linear16"),
            ("sample_rate", "16000"),
            ("channels", "1"),
            ("keywords", "hello,world"),
        ]);

        let url = UpstreamUrlBuilder::new(base)
            .default_params(defaults)
            .client_params(&client)
            .build();

        let params = get_query_params(&url);

        assert_eq!(
            params.get("model"),
            Some(&"nova-3".to_string()),
            "client model should override default nova-3-general"
        );
        assert_eq!(
            params.get("mip_opt_out"),
            Some(&"true".to_string()),
            "client mip_opt_out should override default false"
        );
        assert_eq!(params.get("encoding"), Some(&"linear16".to_string()));
        assert_eq!(params.get("sample_rate"), Some(&"16000".to_string()));
        assert_eq!(params.get("channels"), Some(&"1".to_string()));
        assert_eq!(
            params.get("keywords"),
            Some(&"hello,world".to_string()),
            "keywords should be passed through"
        );
        assert_eq!(params.len(), 6);
    }

    #[test]
    fn test_no_duplicate_params() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let defaults: &[(&str, &str)] = &[("model", "nova-3-general"), ("mip_opt_out", "false")];
        let client = make_params(&[("model", "nova-3"), ("mip_opt_out", "true")]);

        let url = UpstreamUrlBuilder::new(base)
            .default_params(defaults)
            .client_params(&client)
            .build();

        let query = url.query().unwrap();
        let model_count = query.matches("model=").count();
        let mip_opt_out_count = query.matches("mip_opt_out=").count();

        assert_eq!(model_count, 1, "model should appear exactly once");
        assert_eq!(
            mip_opt_out_count, 1,
            "mip_opt_out should appear exactly once"
        );
    }

    #[test]
    fn test_multi_value_params() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let client = make_params_multi(&[
            ("encoding", vec!["linear16"]),
            ("keywords", vec!["hello", "world", "test"]),
        ]);

        let url = UpstreamUrlBuilder::new(base).client_params(&client).build();

        let params = get_query_params_multi(&url);
        assert_eq!(params.get("encoding"), Some(&vec!["linear16".to_string()]));
        assert_eq!(
            params.get("keywords"),
            Some(&vec![
                "hello".to_string(),
                "world".to_string(),
                "test".to_string()
            ])
        );
    }

    #[test]
    fn test_multi_value_overrides_default() {
        let base: url::Url = "wss://api.deepgram.com/v1/listen".parse().unwrap();
        let defaults: &[(&str, &str)] = &[("keywords", "default")];
        let client = make_params_multi(&[("keywords", vec!["hello", "world"])]);

        let url = UpstreamUrlBuilder::new(base)
            .default_params(defaults)
            .client_params(&client)
            .build();

        let params = get_query_params_multi(&url);
        assert_eq!(
            params.get("keywords"),
            Some(&vec!["hello".to_string(), "world".to_string()]),
            "multi-value should completely override default"
        );
    }
}
