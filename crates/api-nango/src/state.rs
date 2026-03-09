use hypr_nango::NangoClient;

use crate::config::{NangoConfig, build_nango_client};
use crate::supabase::SupabaseClient;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: NangoConfig,
    pub(crate) nango: NangoClient,
    pub(crate) supabase: SupabaseClient,
}

impl AppState {
    pub(crate) fn new(config: NangoConfig) -> Self {
        let nango = build_nango_client(&config).expect("failed to build NangoClient");

        let supabase = SupabaseClient::new(
            &config.supabase_url,
            &config.supabase_anon_key,
            config.supabase_service_role_key.clone(),
        );

        Self {
            config,
            nango,
            supabase,
        }
    }
}
