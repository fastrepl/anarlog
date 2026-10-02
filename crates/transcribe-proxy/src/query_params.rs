use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use anlg_language::Language;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};

#[derive(Debug, Clone)]
pub enum QueryValue {
    Single(String),
    Multi(Vec<String>),
}

impl QueryValue {
    pub fn first(&self) -> Option<&str> {
        match self {
            QueryValue::Single(s) => Some(s),
            QueryValue::Multi(v) => v.first().map(|s| s.as_str()),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        match self {
            QueryValue::Single(s) => QueryValueIter::Single(Some(s.as_str())),
            QueryValue::Multi(v) => QueryValueIter::Multi(v.iter().map(|s| s.as_str())),
        }
    }
}

enum QueryValueIter<'a, I: Iterator<Item = &'a str>> {
    Single(Option<&'a str>),
    Multi(I),
}

impl<'a, I: Iterator<Item = &'a str>> Iterator for QueryValueIter<'a, I> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            QueryValueIter::Single(opt) => opt.take(),
            QueryValueIter::Multi(iter) => iter.next(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct QueryParams(HashMap<String, QueryValue>);

impl QueryParams {
    pub fn get_first(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.first())
    }

    pub fn remove(&mut self, key: &str) -> Option<QueryValue> {
        self.0.remove(key)
    }

    pub fn remove_first(&mut self, key: &str) -> Option<String> {
        self.0.remove(key).map(|v| match v {
            QueryValue::Single(s) => s,
            QueryValue::Multi(mut v) => v.remove(0),
        })
    }

    pub fn get_languages(&self) -> Vec<Language> {
        self.get("language")
            .or_else(|| self.get("languages"))
            .map(|v| {
                v.iter()
                    .flat_map(|s| s.split(','))
                    .filter_map(|lang| Language::from_str(lang.trim()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn parse_keywords(&self) -> Vec<String> {
        self.get("keyword")
            .or_else(|| self.get("keywords"))
            .map(|v| {
                v.iter()
                    .flat_map(|s| s.split(','))
                    .map(|k| k.trim().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn parse_optional_u32(&self, key: &str) -> Option<u32> {
        self.get_first(key)
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
    }
}

impl Deref for QueryParams {
    type Target = HashMap<String, QueryValue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for QueryParams {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<S> FromRequestParts<S> for QueryParams
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();

        let raw: HashMap<String, Vec<String>> = serde_html_form::from_str(query).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("failed_to_parse_query_string: {}", e),
            )
                .into_response()
        })?;

        let params = raw
            .into_iter()
            .filter_map(|(k, v)| {
                let value = match v.len() {
                    0 => return None,
                    1 => QueryValue::Single(v.into_iter().next().unwrap()),
                    _ => QueryValue::Multi(v),
                };
                Some((k, value))
            })
            .collect();

        Ok(QueryParams(params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_language::ISO639;
    use axum::http::Uri;

    fn parse_query(query: &str) -> QueryParams {
        let uri: Uri = format!("http://example.com{}", query).parse().unwrap();
        let query_str = uri.query().unwrap_or_default();
        let raw: HashMap<String, Vec<String>> = serde_html_form::from_str(query_str).unwrap();

        let params = raw
            .into_iter()
            .filter_map(|(k, v)| {
                let value = match v.len() {
                    0 => return None,
                    1 => QueryValue::Single(v.into_iter().next().unwrap()),
                    _ => QueryValue::Multi(v),
                };
                Some((k, value))
            })
            .collect();

        QueryParams(params)
    }

    #[test]
    fn parses_single_multi_and_empty_queries() {
        let params = parse_query("?foo=hello");
        assert!(matches!(params.get("foo"), Some(QueryValue::Single(s)) if s == "hello"));

        let params = parse_query("?value=one&value=two");
        assert!(
            matches!(params.get("value"), Some(QueryValue::Multi(v)) if v == &vec!["one", "two"])
        );

        let params = parse_query("");
        assert!(params.is_empty());

        let params = parse_query("?");
        assert!(params.is_empty());

        let params = parse_query("?single=one&multi=a&multi=b&another=value");

        assert!(matches!(params.get("single"), Some(QueryValue::Single(s)) if s == "one"));
        assert!(matches!(params.get("multi"), Some(QueryValue::Multi(v)) if v == &vec!["a", "b"]));
        assert!(matches!(params.get("another"), Some(QueryValue::Single(s)) if s == "value"));
    }

    #[test]
    fn get_first_and_remove_first() {
        let params = parse_query("?foo=hello&bar=one&bar=two");
        assert_eq!(params.get_first("foo"), Some("hello"));
        assert_eq!(params.get_first("bar"), Some("one"));

        let params = parse_query("?foo=hello");
        assert_eq!(params.get_first("missing"), None);

        let mut params = parse_query("?foo=hello");
        assert_eq!(params.remove_first("foo"), Some("hello".to_string()));
        assert!(params.get("foo").is_none());

        let mut params = parse_query("?value=one&value=two&value=three");
        assert_eq!(params.remove_first("value"), Some("one".to_string()));
        assert!(params.get("value").is_none());
    }

    #[test]
    fn get_languages_normalizes_aliases_commas_regions_and_drops_invalid() {
        let params = parse_query("?language=en");
        let languages = params.get_languages();
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].iso639(), ISO639::En);

        let params = parse_query("?language=en&language=ko");
        let languages = params.get_languages();
        assert_eq!(languages.len(), 2);
        assert_eq!(languages[0].iso639(), ISO639::En);
        assert_eq!(languages[1].iso639(), ISO639::Ko);

        let params = parse_query("?language=en,ko,ja");
        let languages = params.get_languages();
        assert_eq!(languages.len(), 3);
        assert_eq!(languages[0].iso639(), ISO639::En);
        assert_eq!(languages[1].iso639(), ISO639::Ko);
        assert_eq!(languages[2].iso639(), ISO639::Ja);

        let params = parse_query("?languages=en");
        let languages = params.get_languages();
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].iso639(), ISO639::En);

        let params = parse_query("?language=en-US,ko-KR");
        let languages = params.get_languages();
        assert_eq!(languages.len(), 2);
        assert_eq!(languages[0].iso639(), ISO639::En);
        assert_eq!(languages[0].region(), Some("US"));
        assert_eq!(languages[1].iso639(), ISO639::Ko);
        assert_eq!(languages[1].region(), Some("KR"));

        let params = parse_query("?language=en,invalid,ko");
        let languages = params.get_languages();
        assert_eq!(languages.len(), 2);
        assert_eq!(languages[0].iso639(), ISO639::En);
        assert_eq!(languages[1].iso639(), ISO639::Ko);
    }

    #[test]
    fn parse_optional_u32_ignores_missing_invalid_and_zero_values() {
        let params = parse_query("?num_speakers=3&min_speakers=0&max_speakers=nope");

        assert_eq!(params.parse_optional_u32("num_speakers"), Some(3));
        assert_eq!(params.parse_optional_u32("min_speakers"), None);
        assert_eq!(params.parse_optional_u32("max_speakers"), None);
        assert_eq!(params.parse_optional_u32("missing"), None);
    }
}
