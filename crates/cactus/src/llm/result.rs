use serde::Deserialize;

fn deserialize_null_as_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompletionResult {
    #[serde(
        default,
        rename = "response",
        deserialize_with = "deserialize_null_as_default"
    )]
    pub text: String,
    #[serde(default)]
    pub cloud_handoff: bool,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub time_to_first_token_ms: f64,
    #[serde(default)]
    pub total_time_ms: f64,
    #[serde(default)]
    pub prefill_tps: f64,
    #[serde(default)]
    pub decode_tps: f64,
    #[serde(default)]
    pub prefill_tokens: u64,
    #[serde(default)]
    pub decode_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}
