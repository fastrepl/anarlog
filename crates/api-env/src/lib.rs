use serde::Deserialize;

pub fn filter_empty<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    Ok(s.filter(|s| !s.is_empty()))
}

pub fn string_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

#[derive(Clone, Deserialize)]
pub struct SupabaseEnv {
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub supabase_service_role_key: String,
}

#[derive(Clone, Deserialize)]
pub struct NangoEnv {
    #[serde(default)]
    pub nango_api_base: Option<String>,
    pub nango_secret_key: String,
}

#[derive(Clone, Deserialize)]
pub struct OpenRouterEnv {
    pub openrouter_api_key: String,
}

#[derive(Clone, Deserialize)]
pub struct StripeEnv {
    pub stripe_secret_key: String,
    pub stripe_monthly_price_id: String,
    pub stripe_yearly_price_id: String,
}

#[derive(Clone, Deserialize)]
pub struct LoopsEnv {
    pub loops_key: String,
}
