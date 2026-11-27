use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const MODEL_KEY_DEFAULT: &str = "default";
pub const MODEL_KEY_TOOL_CALLING: &str = "tool_calling";
pub const MODEL_KEY_AUDIO: &str = "audio";
const MODEL_LATEST_SONNET: &str = "~anthropic/claude-sonnet-latest";

/// OpenRouter models a client may request by id. Any other requested model
/// falls back to the task's default routing.
pub const SELECTABLE_MODELS: &[&str] = &[
    MODEL_LATEST_SONNET,
    "~anthropic/claude-opus-latest",
    "~anthropic/claude-haiku-latest",
    "~openai/gpt-sol-latest",
    "~openai/gpt-astra-latest",
    "~openai/gpt-terra-latest",
    "~openai/gpt-luna-latest",
    "~google/gemini-pro-latest",
    "~google/gemini-flash-latest",
    "~x-ai/grok-latest",
    "~moonshotai/kimi-latest",
    "~deepseek/deepseek-pro-latest",
    "~deepseek/deepseek-flash-latest",
    "~z-ai/glm-latest",
    "~z-ai/glm-flash-latest",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct SelectableModel {
    pub id: &'static str,
    pub label: &'static str,
    pub input_modalities: &'static [&'static str],
    pub object: &'static str,
    pub owned_by: &'static str,
}

/// Hosted Anarlog Pro selectable models in display order, starting with `"Auto"`.
pub const ANARLOG_MODELS: &[SelectableModel] = &[
    SelectableModel {
        id: "Auto",
        label: "Pro (Cloud)",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "anarlog",
    },
    SelectableModel {
        id: MODEL_LATEST_SONNET,
        label: "Claude Sonnet",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "anthropic",
    },
    SelectableModel {
        id: "~anthropic/claude-opus-latest",
        label: "Claude Opus",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "anthropic",
    },
    SelectableModel {
        id: "~anthropic/claude-haiku-latest",
        label: "Claude Haiku",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "anthropic",
    },
    SelectableModel {
        id: "~openai/gpt-sol-latest",
        label: "GPT Sol",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "openai",
    },
    SelectableModel {
        id: "~openai/gpt-astra-latest",
        label: "GPT Astra",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "openai",
    },
    SelectableModel {
        id: "~openai/gpt-terra-latest",
        label: "GPT Terra",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "openai",
    },
    SelectableModel {
        id: "~openai/gpt-luna-latest",
        label: "GPT Luna",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "openai",
    },
    SelectableModel {
        id: "~google/gemini-pro-latest",
        label: "Gemini Pro",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "google",
    },
    SelectableModel {
        id: "~google/gemini-flash-latest",
        label: "Gemini Flash",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "google",
    },
    SelectableModel {
        id: "~x-ai/grok-latest",
        label: "Grok",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "x-ai",
    },
    SelectableModel {
        id: "~moonshotai/kimi-latest",
        label: "Kimi",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "moonshotai",
    },
    SelectableModel {
        id: "~deepseek/deepseek-pro-latest",
        label: "DeepSeek Pro",
        input_modalities: &["text"],
        object: "model",
        owned_by: "deepseek",
    },
    SelectableModel {
        id: "~deepseek/deepseek-flash-latest",
        label: "DeepSeek Flash",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "deepseek",
    },
    SelectableModel {
        id: "~z-ai/glm-latest",
        label: "GLM",
        input_modalities: &["text"],
        object: "model",
        owned_by: "z-ai",
    },
    SelectableModel {
        id: "~z-ai/glm-flash-latest",
        label: "GLM Flash",
        input_modalities: &["text", "image"],
        object: "model",
        owned_by: "z-ai",
    },
];

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
    pub requested_model: Option<String>,
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
                "google/gemini-3.6-flash".into(),
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

    fn resolve_defaults(&self, ctx: &ModelContext) -> Vec<String> {
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

impl ModelResolver for StaticModelResolver {
    fn resolve(&self, ctx: &ModelContext) -> Vec<String> {
        if ctx.has_audio
            && let Some(models) = self.models.get(MODEL_KEY_AUDIO)
        {
            return models.clone();
        }

        let defaults = self.resolve_defaults(ctx);
        match ctx
            .requested_model
            .as_deref()
            .filter(|model| SELECTABLE_MODELS.contains(model))
        {
            Some(requested) => std::iter::once(requested.to_owned())
                .chain(defaults.into_iter().filter(|model| model != requested))
                .collect(),
            None => defaults,
        }
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
                    "google/gemini-3.6-flash",
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
                    "google/gemini-3.6-flash",
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
                    requested_model: None,
                    needs_tool_calling: *needs_tool_calling,
                    has_audio: *has_audio,
                },
                expected,
            );
        }
    }
    #[test]
    fn requested_model_is_used_only_when_selectable() {
        let ctx = |requested: Option<&str>, has_audio: bool| ModelContext {
            task: Some(CharTask::Enhance),
            requested_model: requested.map(str::to_owned),
            needs_tool_calling: false,
            has_audio,
        };
        let resolver = StaticModelResolver::default();

        assert_eq!(
            resolver.resolve(&ctx(Some("~anthropic/claude-opus-latest"), false)),
            vec!["~anthropic/claude-opus-latest", MODEL_LATEST_SONNET],
        );
        assert_eq!(
            resolver.resolve(&ctx(Some("~deepseek/deepseek-pro-latest"), false)),
            vec!["~deepseek/deepseek-pro-latest", MODEL_LATEST_SONNET],
        );
        assert_eq!(
            resolver.resolve(&ctx(Some(MODEL_LATEST_SONNET), false)),
            vec![MODEL_LATEST_SONNET],
        );
        assert_eq!(
            resolver.resolve(&ctx(Some("openai/gpt-5.5-pro"), false)),
            vec![MODEL_LATEST_SONNET],
        );
        assert_eq!(
            resolver.resolve(&ctx(Some("Auto"), false)),
            vec![MODEL_LATEST_SONNET],
        );
        assert_eq!(
            resolver.resolve(&ctx(Some("~anthropic/claude-opus-latest"), true)),
            resolver.resolve(&ctx(None, true)),
        );
    }
}
