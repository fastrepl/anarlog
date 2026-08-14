use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

pub const MAX_PROVIDER_METADATA_ENTRIES: usize = 32;
pub const MAX_PROVIDER_METADATA_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ProviderMetadata(BTreeMap<String, Value>);

impl ProviderMetadata {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.0.iter()
    }

    pub fn into_inner(self) -> BTreeMap<String, Value> {
        self.0
    }
}

impl TryFrom<BTreeMap<String, Value>> for ProviderMetadata {
    type Error = MetadataError;

    fn try_from(value: BTreeMap<String, Value>) -> Result<Self, Self::Error> {
        if value.len() > MAX_PROVIDER_METADATA_ENTRIES {
            return Err(MetadataError::TooManyEntries {
                count: value.len(),
                max: MAX_PROVIDER_METADATA_ENTRIES,
            });
        }

        let bytes = serde_json::to_vec(&value)
            .map_err(|error| MetadataError::Serialization(error.to_string()))?
            .len();
        if bytes > MAX_PROVIDER_METADATA_BYTES {
            return Err(MetadataError::TooLarge {
                bytes,
                max: MAX_PROVIDER_METADATA_BYTES,
            });
        }

        Ok(Self(value))
    }
}

impl<'de> Deserialize<'de> for ProviderMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = BTreeMap::<String, Value>::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MetadataError {
    #[error("provider metadata has {count} entries; maximum is {max}")]
    TooManyEntries { count: usize, max: usize },
    #[error("provider metadata is {bytes} bytes; maximum is {max}")]
    TooLarge { bytes: usize, max: usize },
    #[error("failed to serialize provider metadata: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bounded_metadata() {
        let metadata = ProviderMetadata::try_from(BTreeMap::from([
            ("provider_bot_id".into(), Value::String("bot-1".into())),
            ("attempt".into(), Value::from(2)),
        ]))
        .unwrap();

        assert_eq!(metadata.get("attempt"), Some(&Value::from(2)));
    }

    #[test]
    fn rejects_too_many_entries() {
        let values: BTreeMap<String, Value> = (0..=MAX_PROVIDER_METADATA_ENTRIES)
            .map(|index| (index.to_string(), Value::Null))
            .collect();

        assert!(matches!(
            ProviderMetadata::try_from(values),
            Err(MetadataError::TooManyEntries { .. })
        ));
    }

    #[test]
    fn rejects_oversized_metadata_during_deserialization() {
        let value = serde_json::json!({ "payload": "x".repeat(MAX_PROVIDER_METADATA_BYTES) });

        let error = serde_json::from_value::<ProviderMetadata>(value).unwrap_err();

        assert!(error.to_string().contains("maximum is"));
    }
}
