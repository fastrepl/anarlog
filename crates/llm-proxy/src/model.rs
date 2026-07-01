use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const MODEL_KEY_DEFAULT: &str = "default";
pub const MODEL_KEY_TOOL_CALLING: &str = "tool_calling";
pub const MODEL_KEY_AUDIO: &str = "audio";
const MODEL_LATEST_SONNET: &str = "~anthropic/claude-sonnet-latest";

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    ToSchema,
    strum::Display,
    strum::EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum CharTask {
    Chat,
    Enhance,
    Title,
}

pub struct ModelContext {
    pub task: Option<CharTask>,
    pub needs_tool_calling: bool,
    pub has_audio: bool,
}

pub trait ModelResolver: Send + Sync {
    fn resolve(&self, ctx: &ModelContext) -> Vec<String>;
}

#[derive(Clone)]
pub struct StaticModelResolver {
    pub(crate) models: HashMap<String, Vec<String>>,
}

impl Default for StaticModelResolver {
    fn default() -> Self {
        let mut models = HashMap::new();

        models.insert(CharTask::Chat.to_string(), vec![MODEL_LATEST_SONNET.into()]);
        models.insert(
            CharTask::Title.to_string(),
            vec![MODEL_LATEST_SONNET.into()],
        );
        models.insert(
            CharTask::Enhance.to_string(),
            vec![MODEL_LATEST_SONNET.into()],
        );
        models.insert(
            MODEL_KEY_TOOL_CALLING.to_owned(),
            vec![MODEL_LATEST_SONNET.into()],
        );
        models.insert(
            MODEL_KEY_DEFAULT.to_owned(),
            vec![MODEL_LATEST_SONNET.into()],
        );
        models.insert(
            MODEL_KEY_AUDIO.to_owned(),
            vec![
                "google/gemini-3.1-pro-preview".into(),
                "google/gemini-3.5-flash".into(),
                "mistralai/voxtral-small-24b-2507".into(),
            ],
        );

        Self { models }
    }
}

impl StaticModelResolver {
    pub fn with_models(mut self, key: impl Into<String>, models: Vec<String>) -> Self {
        self.models.insert(key.into(), models);
        self
    }
}

impl ModelResolver for StaticModelResolver {
    fn resolve(&self, ctx: &ModelContext) -> Vec<String> {
        if ctx.has_audio
            && let Some(models) = self.models.get(MODEL_KEY_AUDIO)
        {
            return models.clone();
        }

        if let Some(models) = ctx.task.and_then(|t| self.models.get(&t.to_string())) {
            return models.clone();
        }

        let key = if ctx.needs_tool_calling {
            MODEL_KEY_TOOL_CALLING
        } else {
            MODEL_KEY_DEFAULT
        };
        self.models.get(key).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type ResolveTestCase = (
        &'static str,
        Option<CharTask>,
        bool,
        bool,
        Option<(&'static str, Vec<&'static str>)>,
        &'static [&'static str],
    );

    fn run_resolve_test(
        name: &str,
        resolver: StaticModelResolver,
        ctx: ModelContext,
        expected: &[&str],
    ) {
        let models = resolver.resolve(&ctx);
        let expected: Vec<String> = expected.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(models, expected, "{name}");
    }

    #[test]
    fn resolve() {
        let cases: &[ResolveTestCase] = &[
            (
                "by_task",
                Some(CharTask::Chat),
                false,
                false,
                None,
                &[MODEL_LATEST_SONNET],
            ),
            (
                "by_tool_calling",
                None,
                true,
                false,
                None,
                &[MODEL_LATEST_SONNET],
            ),
            ("default", None, false, false, None, &[MODEL_LATEST_SONNET]),
            (
                "task_overrides_tool_calling",
                Some(CharTask::Chat),
                true,
                false,
                None,
                &[MODEL_LATEST_SONNET],
            ),
            (
                "with_models_custom_key",
                Some(CharTask::Enhance),
                false,
                false,
                Some(("enhance", vec!["foo/bar"])),
                &["foo/bar"],
            ),
            (
                "enhance_uses_quality_models",
                Some(CharTask::Enhance),
                false,
                false,
                None,
                &[MODEL_LATEST_SONNET],
            ),
            (
                "audio_overrides_task",
                Some(CharTask::Chat),
                false,
                true,
                None,
                &[
                    "google/gemini-3.1-pro-preview",
                    "google/gemini-3.5-flash",
                    "mistralai/voxtral-small-24b-2507",
                ],
            ),
            (
                "audio_overrides_tool_calling",
                None,
                true,
                true,
                None,
                &[
                    "google/gemini-3.1-pro-preview",
                    "google/gemini-3.5-flash",
                    "mistralai/voxtral-small-24b-2507",
                ],
            ),
        ];

        for (name, task, needs_tool_calling, has_audio, with_models, expected) in cases {
            let mut resolver = StaticModelResolver::default();
            if let Some((key, models)) = with_models {
                resolver =
                    resolver.with_models(*key, models.iter().map(|s| (*s).to_string()).collect());
            }
            run_resolve_test(
                name,
                resolver,
                ModelContext {
                    task: *task,
                    needs_tool_calling: *needs_tool_calling,
                    has_audio: *has_audio,
                },
                expected,
            );
        }
    }
}
