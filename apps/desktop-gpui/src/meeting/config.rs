use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use anlg_listener_core::{TranscriptionMode, actors::SessionParams};
use anlg_listener2_core::{BatchParams, BatchProvider};
use desktop_runtime::{Result, RuntimeHandle, SessionId};
use futures::future::BoxFuture;
use serde_json::{Value, json};

use super::{
    capture::{CaptureConfig, StartResolver},
    model::{Retention, failure},
    recovery::RecoveryResolver,
};
use crate::product::settings;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Stt,
    Llm,
}

impl ProviderKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Stt => "stt",
            Self::Llm => "llm",
        }
    }
}

pub struct CloudAccess {
    pub access_token: String,
    pub user_id: String,
    pub is_paid: bool,
}

pub struct LocalRequest {
    pub model: String,
    pub file: Option<PathBuf>,
    pub batch: bool,
}

pub type CloudResolver = Arc<dyn Fn() -> BoxFuture<'static, Result<CloudAccess>> + Send + Sync>;
pub type LocalResolver =
    Arc<dyn Fn(LocalRequest) -> BoxFuture<'static, Result<String>> + Send + Sync>;
pub type SecretResolver =
    Arc<dyn Fn(ProviderKind, String) -> BoxFuture<'static, Result<Option<String>>> + Send + Sync>;

#[derive(Clone)]
pub struct ProviderServices {
    pub runtime: RuntimeHandle,
    pub api_url: String,
    pub cloud: CloudResolver,
    pub local: LocalResolver,
    pub secret: SecretResolver,
    pub secret_write: super::subscription::SecretWriter,
}

pub struct Connection {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub self_human_id: Option<String>,
}

pub struct Preferences {
    values: settings::Snapshot,
    providers: BTreeMap<String, Value>,
    legacy: Value,
}

impl Preferences {
    pub fn value(&self, key: &str) -> &Value {
        self.values.get(key).map_or(&Value::Null, |v| &v.value)
    }

    pub fn text(&self, key: &str) -> String {
        self.value(key).as_str().unwrap_or_default().trim().into()
    }

    pub fn strings(&self, key: &str) -> Result<Vec<String>> {
        let value = self.value(key);
        match value {
            Value::String(text) if text.is_empty() => Ok(Vec::new()),
            Value::String(text) => serde_json::from_str(text).map_err(failure),
            Value::Array(_) => serde_json::from_value(value.clone()).map_err(failure),
            _ => Ok(Vec::new()),
        }
    }

    pub fn languages(&self) -> Result<Vec<anlg_language::Language>> {
        let mut values = self.strings("spoken_languages")?;
        if values.is_empty() {
            let language = self.text("ai_language");
            if !language.is_empty() {
                values.push(language);
            }
        }
        values.sort();
        values.dedup();
        values
            .into_iter()
            .map(|value| value.parse().map_err(failure))
            .collect()
    }

    fn provider(&self, kind: ProviderKind, provider: &str) -> &Value {
        self.providers
            .get(&format!("ai_provider:{}:{provider}", kind.key()))
            .unwrap_or(&self.legacy["ai"][kind.key()][provider])
    }
}

pub async fn preferences(runtime: &RuntimeHandle) -> Result<Preferences> {
    runtime.submit(|services| async move {
        let rows = services.executor.execute(
            "SELECT id, CASE WHEN length(CAST(value_json AS BLOB)) <= 524288 THEN value_json END AS value_json, length(CAST(value_json AS BLOB)) > 524288 AS oversized, 0 AS source_rank FROM app_settings WHERE id NOT LIKE 'capture_%' AND id NOT LIKE 'gpui_%' UNION ALL SELECT id, CASE WHEN length(CAST(value_json AS BLOB)) <= 524288 THEN value_json END, length(CAST(value_json AS BLOB)) > 524288, 1 FROM synced_preferences".into(),
            vec![],
        ).await.map_err(failure)?;
        let values = settings::decode(&rows)?;
        let mut providers = BTreeMap::new();
        let mut legacy = Value::Null;
        for row in &rows {
            let id = row["id"].as_str().unwrap_or_default();
            if row["source_rank"] != 0 || (!id.starts_with("ai_provider:") && id != "legacy_settings_document") { continue; }
            let value = serde_json::from_str(row["value_json"].as_str().unwrap_or("null")).map_err(failure)?;
            if id == "legacy_settings_document" { legacy = value; } else { providers.insert(id.into(), value); }
        }
        Ok(Preferences { values, providers, legacy })
    })?.receive().await
}

impl ProviderServices {
    pub fn ai_services(
        &self,
        capture_active: super::ai::CaptureActivity,
    ) -> Result<super::ai_view::AiViewServices> {
        let approvals = super::tools::Approvals::default();
        Ok(super::ai_view::AiViewServices {
            ai: Arc::new(super::ai::AiServices::new(
                self.runtime.clone(),
                self.ai_resolver(),
                super::tools::executor(self.runtime.clone(), approvals.clone()),
                capture_active,
            )?),
            context: super::context::resolver(self.runtime.clone()),
            approvals,
        })
    }

    pub fn start_resolver(&self) -> StartResolver {
        let services = self.clone();
        Arc::new(move |session| {
            let services = services.clone();
            Box::pin(async move { services.capture(session).await })
        })
    }

    pub fn recovery_resolver(&self) -> RecoveryResolver {
        let services = self.clone();
        Arc::new(move |session, path| {
            let services = services.clone();
            Box::pin(async move { services.batch(session, path).await })
        })
    }

    pub async fn connection(
        &self,
        preferences: &Preferences,
        kind: ProviderKind,
        batch: bool,
    ) -> Result<Connection> {
        let provider = preferences.text(&format!("current_{}_provider", kind.key()));
        let model = preferences.text(&format!("current_{}_model", kind.key()));
        if provider.is_empty() || model.is_empty() {
            return Err(failure("Choose a provider and model in settings."));
        }
        let config = preferences.provider(kind, &provider);
        let mut connection = Connection {
            provider,
            model,
            base_url: String::new(),
            api_key: String::new(),
            self_human_id: None,
        };
        let local = matches!(kind, ProviderKind::Stt)
            && is_local(&connection.provider, &connection.model)
            || matches!(kind, ProviderKind::Llm) && connection.provider == "local";
        if local {
            let file = if connection.provider == "local_file" || connection.model == "local-file" {
                let path = preferences.text("local_stt_model_path");
                if path.is_empty() {
                    return Err(failure("Choose a local transcription model file."));
                }
                Some(PathBuf::from(path))
            } else {
                None
            };
            connection.base_url = (self.local)(LocalRequest {
                model: connection.model.clone(),
                file,
                batch,
            })
            .await?;
            if connection.base_url.is_empty() {
                return Err(failure("The local model did not return a ready endpoint."));
            }
            if matches!(kind, ProviderKind::Llm) {
                connection.provider = "custom".into();
            }
            return Ok(connection);
        }
        if connection.provider == "anarlog" || connection.provider == "hyprnote" {
            let access = (self.cloud)().await?;
            if access.access_token.is_empty() {
                return Err(failure("Sign in to use Anarlog AI."));
            }
            if !access.is_paid {
                return Err(failure("Anarlog AI requires an active paid plan or trial."));
            }
            // Hosted credentials only travel to the configured first-party API.
            connection.base_url = format!("{}/{}", self.api_url.trim_end_matches('/'), kind.key());
            connection.api_key = access.access_token;
            connection.self_human_id = Some(access.user_id);
            connection.provider = "anarlog".into();
        } else {
            connection.base_url = config["base_url"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .into();
            if connection.base_url.is_empty() {
                connection.base_url = match kind {
                    ProviderKind::Stt => stt_base(&connection.provider),
                    ProviderKind::Llm => super::provider::default_base(&connection.provider),
                }
                .ok_or_else(|| failure("Configure this provider's base URL."))?;
            }
            connection.api_key = (self.secret)(kind, connection.provider.clone())
                .await?
                .unwrap_or_else(|| config["api_key"].as_str().unwrap_or_default().into());
            if connection.api_key.trim().is_empty()
                && (matches!(kind, ProviderKind::Stt)
                    || !matches!(
                        connection.provider.as_str(),
                        "ollama" | "lmstudio" | "custom" | "apple_foundation"
                    ))
            {
                return Err(failure("Save this provider's API key in secure settings."));
            }
        }
        let url = reqwest::Url::parse(&connection.base_url)
            .map_err(|_| failure("Invalid provider URL."))?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(failure(
                "Provider URL must use HTTP(S) without embedded credentials.",
            ));
        }
        Ok(connection)
    }

    pub async fn capture(&self, session: SessionId) -> Result<CaptureConfig> {
        let preferences = preferences(&self.runtime).await?;
        let connection = self
            .connection(&preferences, ProviderKind::Stt, false)
            .await?;
        let (memo, participants, mut keywords) = self.session_hints(session.clone()).await?;
        keywords.extend(preferences.strings("personalization_dictionary_terms")?);
        normalize_keywords(&mut keywords);
        let retention = Retention::from_setting(preferences.value("audio_retention"))?;
        let mut params = SessionParams {
            session_id: session.0.to_string(),
            retain_audio: Some(retention != Retention::Never),
            languages: preferences.languages()?,
            onboarding: false,
            transcription_mode: if connection.provider == "local_file"
                || connection.model == "local-file"
            {
                TranscriptionMode::Batch
            } else {
                TranscriptionMode::Live
            },
            model: connection.model,
            base_url: connection.base_url,
            api_key: connection.api_key,
            keywords,
            mic_device: Some(preferences.text("microphone_device")).filter(|s| !s.is_empty()),
            participant_human_ids: participants,
            self_human_id: connection.self_human_id,
            speaker_assignments: Vec::new(),
        };
        params.transcription_mode = params.effective_transcription_mode();
        Ok(CaptureConfig {
            params,
            provider: connection.provider,
            retention,
            memo,
            recovery: Some(self.recovery_resolver()),
        })
    }

    pub async fn batch(&self, session: SessionId, path: PathBuf) -> Result<BatchParams> {
        let preferences = preferences(&self.runtime).await?;
        let connection = self
            .connection(&preferences, ProviderKind::Stt, true)
            .await?;
        let (_, _, mut keywords) = self.session_hints(session.clone()).await?;
        keywords.extend(preferences.strings("personalization_dictionary_terms")?);
        normalize_keywords(&mut keywords);
        let provider = batch_provider(&connection.provider, &connection.model)?;
        let languages = preferences.languages()?;
        if let Some(adapter) = provider.to_adapter_kind()
            && !adapter.is_supported_languages_batch(&languages, Some(&connection.model))
        {
            return Err(failure(
                "Selected batch model does not support the meeting languages.",
            ));
        }
        Ok(BatchParams {
            session_id: session.0.to_string(),
            provider,
            file_path: path
                .to_str()
                .ok_or_else(|| failure("Audio path is not valid UTF-8."))?
                .into(),
            model: Some(connection.model),
            base_url: connection.base_url,
            api_key: connection.api_key,
            languages,
            keywords,
            num_speakers: None,
            min_speakers: None,
            max_speakers: None,
            known_speakers: Vec::new(),
        })
    }

    async fn session_hints(
        &self,
        session: SessionId,
    ) -> Result<(String, Vec<String>, Vec<String>)> {
        self.runtime.submit(move |services| async move {
            let rows = services.executor.execute(
                "SELECT s.title, COALESCE(d.body, '') AS memo, COALESCE(d.body_format, 'prosemirror_json') AS body_format FROM sessions s LEFT JOIN session_documents d ON d.session_id = s.id AND d.kind = 'note' AND d.deleted_at IS NULL WHERE s.id = ? AND s.deleted_at IS NULL AND s.locked = 0 LIMIT 1".into(),
                vec![json!(session)],
            ).await.map_err(failure)?;
            let row = rows.first().ok_or_else(|| failure("Meeting is missing or locked."))?;
            let memo = super::context::markdown(row["memo"].as_str().unwrap_or_default(), row["body_format"].as_str().unwrap_or_default())?;
            let mut keywords = vec![row["title"].as_str().unwrap_or_default().into()];
            let participants = services.executor.execute(
                "SELECT p.human_id, COALESCE(NULLIF(h.name, ''), p.display_name) AS name FROM session_participants p LEFT JOIN humans h ON h.id = p.human_id AND h.deleted_at IS NULL WHERE p.session_id = ? AND p.source <> 'excluded' AND p.deleted_at IS NULL ORDER BY p.id LIMIT 500".into(), vec![json!(session)]
            ).await.map_err(failure)?;
            let mut ids = Vec::new();
            for participant in participants {
                if let Some(id) = participant["human_id"].as_str().filter(|id| !id.is_empty()) { ids.push(id.into()); }
                if let Some(name) = participant["name"].as_str() { keywords.push(name.into()); }
            }
            Ok((memo, ids, keywords))
        })?.receive().await
    }
}

fn normalize_keywords(values: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    values.retain_mut(|value| {
        *value = value.trim().to_owned();
        !value.is_empty() && value.len() <= 256 && seen.insert(value.to_lowercase())
    });
    values.truncate(50);
}

fn is_local(provider: &str, model: &str) -> bool {
    matches!(
        provider,
        "local_file" | "soniqo" | "apple_speech" | "apple-speech"
    ) || (matches!(provider, "anarlog" | "hyprnote")
        && (model.starts_with("soniqo-")
            || model.starts_with("am-")
            || model == "apple-speech"
            || model == "local-file"))
}

fn batch_provider(provider: &str, model: &str) -> Result<BatchProvider> {
    if provider == "local_file" || model == "local-file" {
        return Ok(BatchProvider::WhisperLocal);
    }
    if is_local(provider, model) {
        return Ok(if model.starts_with("soniqo-") {
            BatchProvider::Soniqo
        } else if model == "apple-speech" {
            BatchProvider::AppleSpeech
        } else {
            BatchProvider::Am
        });
    }
    match provider {
        "custom" | "cloudflare_workers_ai" => Ok(BatchProvider::Deepgram),
        "hyprnote" => Ok(BatchProvider::Anarlog),
        provider => provider.parse().map_err(failure),
    }
}

fn stt_base(provider: &str) -> Option<String> {
    let base = match provider {
        "assemblyai" => "https://api.assemblyai.com",
        "gladia" => "https://api.gladia.io",
        "openrouter" => "https://openrouter.ai/api/v1",
        "siliconflow" => "https://api.siliconflow.com/v1",
        "zai" => "https://api.z.ai/api/paas/v4",
        "wisprflow" => "https://platform-api.wisprflow.ai",
        "azure_speech" | "aws_transcribe" | "cloudflare_workers_ai" | "custom" => return None,
        value => {
            return value
                .parse::<owhisper_client::Provider>()
                .ok()
                .map(|p| p.default_api_base().into());
        }
    };
    Some(base.into())
}

pub fn keyring_secrets(identifier: String) -> SecretResolver {
    Arc::new(move |kind, provider| {
        let identifier = identifier.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let canonical = canonical_identifier(&identifier);
                let account = format!("ai-provider-api-keys:{}:{provider}", kind.key());
                let current = if identifier == "com.hyprnote.dev" {
                    format!("v2:{account}")
                } else {
                    account.clone()
                };
                let mut locations = vec![(format!("{canonical}.secure-store"), current.clone())];
                if current != account {
                    locations.push((format!("{canonical}.secure-store"), account.clone()));
                }
                if canonical != identifier {
                    locations.push((format!("{identifier}.secure-store"), account));
                }
                for (service, account) in locations {
                    let entry = keyring::Entry::new(&service, &account)
                        .map_err(|_| failure("Could not open the secure provider key store."))?;
                    match entry.get_password() {
                        Ok(secret) => return Ok(Some(secret)),
                        Err(keyring::Error::NoEntry) => {}
                        Err(_) => {
                            return Err(failure(
                                "Unlock or repair your system keyring to read the provider key.",
                            ));
                        }
                    }
                }
                Ok(None)
            })
            .await
            .map_err(failure)?
        })
    })
}

pub(super) fn canonical_identifier(identifier: &str) -> &str {
    match identifier {
        "com.hyprnote.dev" => "com.anarlog.dev",
        "com.hyprnote.staging" => "com.anarlog.staging",
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" => "com.anarlog.stable",
        identifier => identifier,
    }
}
