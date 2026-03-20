use std::sync::Arc;

#[derive(Clone)]
pub struct SyncConfig {
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub auth: Option<Arc<hypr_supabase_auth::server::SupabaseAuth>>,
}

impl SyncConfig {
    pub fn new(supabase_url: impl Into<String>, supabase_anon_key: impl Into<String>) -> Self {
        Self {
            supabase_url: supabase_url.into(),
            supabase_anon_key: supabase_anon_key.into(),
            auth: None,
        }
    }

    pub fn with_auth(mut self, auth: Arc<hypr_supabase_auth::server::SupabaseAuth>) -> Self {
        self.auth = Some(auth);
        self
    }
}
