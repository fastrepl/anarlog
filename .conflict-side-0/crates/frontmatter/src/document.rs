use std::str::FromStr;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_yaml::Value;

use crate::Error;

const DELIMITER: &str = "---";

fn strip_line_ending(s: &str) -> &str {
    s.strip_prefix("\r\n")
        .or_else(|| s.strip_prefix('\n'))
        .unwrap_or(s)
}

fn find_closing_delimiter(s: &str) -> Option<usize> {
    s.find(&format!("\r\n{}", DELIMITER))
        .or_else(|| s.find(&format!("\n{}", DELIMITER)))
        .or_else(|| {
            if s.starts_with(DELIMITER) {
                Some(0)
            } else {
                None
            }
        })
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Mapping(mapping) => {
            let mut entries: Vec<_> = mapping.into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| {
                let a_str = value_to_sort_key(a);
                let b_str = value_to_sort_key(b);
                a_str.cmp(&b_str)
            });
            let mut sorted = serde_yaml::Mapping::new();
            for (k, v) in entries {
                sorted.insert(k, sort_value(v));
            }
            Value::Mapping(sorted)
        }
        Value::Sequence(seq) => Value::Sequence(seq.into_iter().map(sort_value).collect()),
        other => other,
    }
}

fn value_to_sort_key(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => serde_yaml::to_string(value).unwrap_or_default(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document<T> {
    pub frontmatter: T,
    pub content: String,
}

impl<T> Document<T> {
    pub fn new(frontmatter: T, content: impl Into<String>) -> Self {
        Self {
            frontmatter,
            content: content.into(),
        }
    }
}

impl<T: DeserializeOwned> FromStr for Document<T> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim_start();

        if !s.starts_with(DELIMITER) {
            return Err(Error::MissingOpeningDelimiter);
        }

        let after_opening = &s[DELIMITER.len()..];
        let after_opening = strip_line_ending(after_opening);

        let closing_pos =
            find_closing_delimiter(after_opening).ok_or(Error::MissingClosingDelimiter)?;

        let yaml_str = &after_opening[..closing_pos];
        let after_closing = &after_opening[closing_pos..];
        let after_closing = strip_line_ending(after_closing);
        let after_closing = after_closing
            .strip_prefix(DELIMITER)
            .unwrap_or(after_closing);

        let frontmatter: T = if yaml_str.trim().is_empty() {
            serde_yaml::from_str("{}")?
        } else {
            serde_yaml::from_str(yaml_str)?
        };

        let content = strip_line_ending(strip_line_ending(after_closing)).to_string();

        Ok(Document {
            frontmatter,
            content,
        })
    }
}

impl<T: Serialize> Document<T> {
    pub fn render(&self) -> Result<String, Error> {
        let value = serde_yaml::to_value(&self.frontmatter)?;
        let sorted = sort_value(value);
        let yaml = serde_yaml::to_string(&sorted)?;
        let mut output = String::new();
        output.push_str(DELIMITER);
        output.push('\n');
        output.push_str(&yaml);
        output.push_str(DELIMITER);
        output.push_str("\n\n");
        output.push_str(&self.content);
        Ok(output)
    }
}

impl<T: Serialize> Serialize for Document<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = self.render().map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&s)
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Document<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Document::from_str(&s).map_err(serde::de::Error::custom)
    }
}
