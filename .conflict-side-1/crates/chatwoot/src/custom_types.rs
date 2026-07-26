use serde::{Deserialize, Deserializer, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ContactMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    #[serde(
        default,
        deserialize_with = "deserialize_option_i64_from_int_or_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub current_page: Option<i64>,
}

fn deserialize_option_i64_from_int_or_string<'de, D>(
    deserializer: D,
) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RawValue {
        Integer(i64),
        String(String),
    }

    let value = Option::<RawValue>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(RawValue::Integer(value)) => Ok(Some(value)),
        Some(RawValue::String(value)) => value
            .parse::<i64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::ContactMeta;

    #[test]
    fn deserializes_integer_current_page() {
        let meta: ContactMeta = serde_json::from_value(serde_json::json!({
            "count": 10,
            "current_page": 2
        }))
        .unwrap();

        assert_eq!(meta.count, Some(10));
        assert_eq!(meta.current_page, Some(2));
    }

    #[test]
    fn deserializes_string_current_page() {
        let meta: ContactMeta = serde_json::from_value(serde_json::json!({
            "count": 10,
            "current_page": "2"
        }))
        .unwrap();

        assert_eq!(meta.current_page, Some(2));
    }
}
