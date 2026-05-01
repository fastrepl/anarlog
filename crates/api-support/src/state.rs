use hypr_api_auth::{AuthContext, AuthState};
use octocrab::Octocrab;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use sqlx::PgPool;
use stripe::Client as StripeClient;

use crate::config::SupportConfig;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: SupportConfig,
    pub(crate) octocrab: Octocrab,
    pub(crate) _db_pool: PgPool,
    pub(crate) stripe: StripeClient,
    pub(crate) _auth: AuthState,
    pub(crate) http_client: HttpClient,
}

impl AppState {
    pub(crate) async fn new(config: SupportConfig) -> Self {
        let key = {
            let pem = config.github.github_bot_private_key.replace("\\n", "\n");
            let pem = if pem.starts_with("-----") {
                pem
            } else {
                format!("-----BEGIN RSA PRIVATE KEY-----\n{pem}\n-----END RSA PRIVATE KEY-----")
            };
            jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes())
                .expect("invalid GitHub App private key")
        };

        let octocrab = Octocrab::builder()
            .app(config.github.github_bot_app_id.into(), key)
            .build()
            .expect("failed to build octocrab client");

        let db_pool = PgPool::connect(&config.support_database.support_database_url)
            .await
            .expect("failed to connect to Supabase Postgres");

        let stripe = StripeClient::new(&config.stripe.stripe_secret_key);

        let auth = config.auth.clone();

        Self {
            config,
            octocrab,
            _db_pool: db_pool,
            stripe,
            _auth: auth,
            http_client: HttpClient::new(),
        }
    }

    pub(crate) async fn installation_client(&self) -> Result<Octocrab, octocrab::Error> {
        self.octocrab
            .installation(self.config.github.github_bot_installation_id.into())
    }

    pub(crate) async fn get_stripe_customer_id(
        &self,
        auth: &AuthContext,
    ) -> Result<Option<String>, String> {
        #[derive(Deserialize)]
        struct Profile {
            stripe_customer_id: Option<String>,
        }

        let url = format!(
            "{}/rest/v1/profiles?select={}&id=eq.{}",
            self.config.supabase.supabase_url,
            "stripe_customer_id",
            urlencoding::encode(&auth.claims.sub),
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", auth.token))
            .header("apikey", &self.config.supabase.supabase_anon_key)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("profiles query failed: {} - {}", status, body));
        }

        let profiles: Vec<Profile> = response.json().await.map_err(|e| e.to_string())?;

        Ok(profiles.first().and_then(|p| p.stripe_customer_id.clone()))
    }
}
