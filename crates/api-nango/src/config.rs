use hypr_api_env::{NangoEnv, SupabaseEnv};
use hypr_nango::NangoClient;

#[derive(Clone)]
pub struct NangoConfig {
    pub nango: NangoEnv,
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub supabase_service_role_key: Option<String>,
}

impl NangoConfig {
    pub fn new(
        nango: &NangoEnv,
        supabase: &SupabaseEnv,
        supabase_service_role_key: Option<String>,
    ) -> Self {
        Self {
            nango: nango.clone(),
            supabase_url: supabase.supabase_url.clone(),
            supabase_anon_key: supabase.supabase_anon_key.clone(),
            supabase_service_role_key,
        }
    }
}

pub(crate) fn build_nango_client(config: &NangoConfig) -> Result<NangoClient, hypr_nango::Error> {
    let mut builder = hypr_nango::NangoClient::builder().api_key(&config.nango.nango_secret_key);
    if let Some(api_base) = &config.nango.nango_api_base {
        builder = builder.api_base(api_base);
    }

    builder.build()
}
