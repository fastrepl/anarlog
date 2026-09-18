//! LLM model listing (`settings/ai/shared/list-*.ts`): each provider's
//! catalogue endpoint, the shared ignore rules (`list-common.ts`), the
//! recency ordering, and the static catalogues, so the Intelligence page
//! offers the same models the Tauri app does.

use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde::Deserialize;

/// `ModelIgnoreReason`
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IgnoreReason {
    CommonKeyword,
    OldModel,
    DateSnapshot,
    NoTool,
    NoTextInput,
    NoCompletion,
    NotLlm,
    NotChatModel,
    ContextTooSmall,
}

impl IgnoreReason {
    /// `formatIgnoreReason`
    pub fn label(self) -> &'static str {
        match self {
            Self::CommonKeyword => "Contains common ignore keyword",
            Self::OldModel => "Old or deprecated model",
            Self::DateSnapshot => "Date-specific snapshot",
            Self::NoTool => "No tool support",
            Self::NoTextInput => "No text input support",
            Self::NoCompletion => "No completion support",
            Self::NotLlm => "Not an LLM type",
            Self::NotChatModel => "Not a chat model",
            Self::ContextTooSmall => "Context length too small",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoredModel {
    pub id: String,
    pub reasons: Vec<IgnoreReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputModality {
    Text,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListModelsResult {
    pub models: Vec<String>,
    pub ignored: Vec<IgnoredModel>,
    /// `metadata[id].input_modalities`, for the ids that have any.
    pub metadata: BTreeMap<String, Vec<InputModality>>,
}

/// `REQUEST_TIMEOUT`
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_MODEL_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

const COMMON_IGNORE_KEYWORDS: &[&str] = &[
    "embed",
    "sora",
    "tts",
    "whisper",
    "dall-e",
    "audio",
    "image",
    "imagine",
    "veo",
    "lyria",
    "computer",
    "robotics",
    "realtime",
    "-live",
    "voice",
    "moderation",
    "codex",
    "transcribe",
    "translate",
    "search-api",
    "search-preview",
    "deep-research",
    "antigravity",
    // Restricted OpenAI programs that only appear for approved orgs.
    "cyber",
    "daybreak",
];

const MODEL_PRIORITY_PATTERNS: &[&str] = &[
    r"(?:^|/)gpt-5\.6(?:-sol)?$",
    r"(?:^|/)gpt-5\.6-terra$",
    r"(?:^|/)gpt-5\.6-luna$",
    r"(?:^|/)gpt-5\.5$",
    r"(?:^|/)chat-latest$",
    r"(?:^|/)claude-fable-5[-.]1$",
    r"(?:^|/)claude-opus-5$",
    r"(?:^|/)claude-sonnet-(?:5|latest)$",
    r"(?:^|/)claude-fable-5$",
    r"(?:^|/)gpt-5\.4$",
    r"(?:^|/)gpt-5\.4-mini$",
    r"(?:^|/)gpt-5\.4-nano$",
    r"(?:^|/)claude-opus-4[-.]8$",
    r"(?:^|/)claude-haiku-4[-.]5(?:-\d{8})?$",
    r"(?:^|/)gemini-3\.8-flash$",
    r"(?:^|/)gemini-3\.7-flash$",
    r"(?:^|/)gemini-3\.6-flash$",
    r"(?:^|/)gemini-3\.5-flash-lite$",
    r"(?:^|/)gemini-3\.1-pro-preview$",
    r"(?:^|/)gemini-3\.5-flash$",
    r"(?:^|/)gemini-3\.1-flash-lite$",
    r"(?:^|/)grok-4\.6$",
    r"(?:^|/)grok-4\.5$",
    r"(?:^|/)grok-4\.3$",
    r"(?:^|/)mistral-medium-(?:3[-.]5|2604)",
    r"(?:^|/)(?:mistral-small-4|mistral-small-latest|mistral-small-2603)",
    r"(?:^|/)(?:mistral-large-3|mistral-large-2512)",
    r"(?:^|/)ministral-(?:3|14b-2512|8b-2512|3b-2512)",
    r"(?:^|/)kimi-k3$",
    r"(?:^|/)deepseek-v4-(?:pro|flash)$",
    r"(?:^|/)glm-5\.3$",
    r"(?:^|/)muse-spark-1\.3$",
];

static PRIORITY: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    MODEL_PRIORITY_PATTERNS
        .iter()
        .map(|pattern| Regex::new(pattern).expect("valid priority pattern"))
        .collect()
});

/// Regexes compiled once; every rule below is a literal port of the
/// JavaScript one, run against the lower-cased model name.
macro_rules! re {
    ($name:ident, $pattern:literal) => {
        static $name: LazyLock<Regex> =
            LazyLock::new(|| Regex::new($pattern).expect("valid pattern"));
    };
}

/// `dottedModelProviders`
const DOTTED_MODEL_PROVIDERS: &[&str] = &[
    "ai21",
    "amazon",
    "anthropic",
    "cohere",
    "deepseek",
    "google",
    "meta",
    "minimax",
    "mistral",
    "moonshot",
    "nvidia",
    "openai",
    "qwen",
    "stability",
    "twelvelabs",
    "writer",
    "xai",
    "zai",
];

/// `modelName`: the lower-cased last path segment, minus a leading `~`,
/// minus a dotted provider prefix (`anthropic.claude-opus-5` → `claude-opus-5`).
pub fn model_name(id: &str) -> String {
    let name = id
        .trim()
        .trim_start_matches('~')
        .to_lowercase()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string();
    let segments: Vec<&str> = name.split('.').collect();
    match segments
        .iter()
        .position(|segment| DOTTED_MODEL_PROVIDERS.contains(segment))
    {
        Some(index) if index < segments.len() - 1 => segments[index + 1..].join("."),
        _ => name,
    }
}

/// `shouldIgnoreCommonKeywords`
pub fn should_ignore_common_keywords(id: &str) -> bool {
    let lower = id.to_lowercase();
    COMMON_IGNORE_KEYWORDS
        .iter()
        .any(|keyword| lower.contains(keyword))
}

re!(DATE_DASHED, r"-\d{4}-\d{2}-\d{2}");
re!(DATE_COMPACT, r"-\d{8}$");
re!(DATE_YEAR, r"-20\d{2}$");

/// `isDateSnapshot`
pub fn is_date_snapshot(id: &str) -> bool {
    DATE_DASHED.is_match(id) || DATE_COMPACT.is_match(id) || DATE_YEAR.is_match(id)
}

re!(NON_CHAT_O, r"^o\d");
re!(NON_CHAT_4O, r"^gpt-4o-");
re!(NON_CHAT_41, r"^gpt-4\.1");
re!(NON_CHAT_BANANA, r"^nano-banana");
re!(NON_CHAT_GROK, r"^grok-(?:build|code-fast)");
re!(NON_CHAT_MULTI, r"multi-agent");

/// `isNonChatModel`
pub fn is_non_chat_model(id: &str) -> bool {
    let lower = id.to_lowercase();
    let name = model_name(id);
    NON_CHAT_O.is_match(&name)
        || NON_CHAT_4O.is_match(&name)
        || NON_CHAT_41.is_match(&name)
        || name.starts_with("ft:")
        || lower.starts_with("ft:")
        || NON_CHAT_BANANA.is_match(&name)
        || NON_CHAT_GROK.is_match(&name)
        || NON_CHAT_MULTI.is_match(&name)
}

re!(NON_STREAMING, r"^gpt-\d+(?:\.\d+)*-pro(?:$|-)");

/// `isNonStreamingModel`
pub fn is_non_streaming_model(id: &str) -> bool {
    NON_STREAMING.is_match(&model_name(id))
}

/// `removeNonStreamingModels`
pub fn remove_non_streaming_models(result: ListModelsResult) -> ListModelsResult {
    ListModelsResult {
        models: result
            .models
            .into_iter()
            .filter(|id| !is_non_streaming_model(id))
            .collect(),
        ignored: result
            .ignored
            .into_iter()
            .filter(|model| !is_non_streaming_model(&model.id))
            .collect(),
        metadata: result
            .metadata
            .into_iter()
            .filter(|(id, _)| !is_non_streaming_model(id))
            .collect(),
    }
}

static OLD_MODEL_RULES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"^gpt-3\.5",
        r"^gpt-4",
        r"^gpt-5($|-)",
        r"^gpt-5\.[1-3]($|-)",
        r"^(davinci|babbage|curie|ada)(-|$)",
        r"^gemini-(1|2)(\.|-)",
        // The Gemini 3.0 previews are deprecated in favour of the 3.x GA line.
        r"^gemini-3-",
        r"^(open-)?mistral-(7b|nemo)(-|$)",
        r"^open-mixtral",
        r"^mistral-large-(24|240|241|2502|2508)",
        r"^mistral-medium-(2505|2508|3[.-]1)($|-)",
        r"^mistral-small-(3|2503|2506)($|-)",
        r"^magistral-(small|medium)-\d{4}($|-)",
        r"^devstral-",
        r"^(pixtral|voxtral-mini-2507|mistral-saba|codestral-2501)",
        r"^ministral-(3b|8b)-24",
        // Retired 2026-05-15; the slugs silently redirect to grok-4.3.
        r"^grok-[23]($|-)",
        r"^grok-4($|-)",
        // deepseek-chat / deepseek-reasoner were discontinued 2026-07-24.
        r"^deepseek-(chat|reasoner)$",
        r"^deepseek-(v3|r1)",
        r"^(kimi-latest|kimi-thinking-preview|moonshot-v1)",
        r"^kimi-k2($|-|[.p]5($|-))",
        r"^glm-4",
        r"^glm-5($|-|[.p]1($|-))",
        r"^command-r(-plus)?(-|$)",
        r"^command(-light)?$",
        r"^c4ai-aya",
        r"^llama-?3[.-]",
        r"^llama-?4-",
        r"^llama-guard",
        r"^qwen-(3|max|plus|flash|turbo)",
        r"^qwen(2|3-)",
        r"^(qwq|qvq)-",
    ]
    .iter()
    .map(|pattern| Regex::new(pattern).expect("valid old-model pattern"))
    .collect()
});

re!(OLD_CLAUDE, r"^claude-(2|3|instant)");
re!(OLD_CLAUDE_OPUS_4, r"^claude-opus-4($|-)");
re!(OLD_CLAUDE_OPUS_4_8, r"^claude-opus-4-8($|-)");
re!(OLD_CLAUDE_SONNET_4, r"^claude-sonnet-4($|-)");
re!(OLD_CLAUDE_HAIKU_4, r"^claude-haiku-4($|-)");
re!(OLD_CLAUDE_HAIKU_4_5, r"^claude-haiku-4-5($|-)");

/// `isOldModel`
pub fn is_old_model(id: &str) -> bool {
    let name = model_name(id);
    let dashed = name.replace('.', "-");
    if OLD_MODEL_RULES.iter().any(|rule| rule.is_match(&name)) {
        return true;
    }
    if OLD_CLAUDE.is_match(&dashed) {
        return true;
    }
    if OLD_CLAUDE_OPUS_4.is_match(&dashed) && !OLD_CLAUDE_OPUS_4_8.is_match(&dashed) {
        return true;
    }
    if OLD_CLAUDE_SONNET_4.is_match(&dashed) {
        return true;
    }
    if OLD_CLAUDE_HAIKU_4.is_match(&dashed) && !OLD_CLAUDE_HAIKU_4_5.is_match(&dashed) {
        return true;
    }
    false
}

/// `sortModelsByRecency`: by the first priority pattern the name matches,
/// then by name (`localeCompare` on ASCII ids is a plain ordering).
pub fn sort_models_by_recency(models: &[String]) -> Vec<String> {
    let priority = |model: &str| {
        let normalized = model_name(model);
        PRIORITY
            .iter()
            .position(|pattern| pattern.is_match(&normalized))
            .unwrap_or(PRIORITY.len())
    };
    let mut sorted: Vec<String> = models.to_vec();
    sorted.sort_by(|a, b| priority(a).cmp(&priority(b)).then_with(|| a.cmp(b)));
    sorted
}

/// `MODEL_NAME_OVERRIDES`
const MODEL_NAME_OVERRIDES: &[(&str, &str)] = &[
    ("chat-latest", "Chat Latest"),
    ("gpt-chat-latest", "GPT Chat Latest"),
    ("claude-sonnet-latest", "Claude Sonnet 5"),
    ("claude-sonnet-5", "Claude Sonnet 5"),
];

re!(
    RELEASE_DATE,
    r"-(?:\d{2}-20\d{2}|20\d{6}|20\d{2}-\d{2}-\d{2}|2\d{3})$"
);
re!(CLAUDE_MODEL, r"^claude-(opus|sonnet|haiku|fable)-(.+)$");
re!(GPT_MODEL, r"^gpt-(.+)$");
re!(VERSION_PAIR, r"\b(\d+) (\d+)\b");

/// `displayLlmModelId`: the human name the combobox shows for a stored id.
pub fn display_llm_model_id(provider_id: &str, model: &str) -> String {
    if provider_id == "anarlog" && model == "Auto" {
        return "Pro (Cloud)".to_string();
    }
    let normalized = RELEASE_DATE.replace(&model_name(model), "").to_string();
    if let Some((_, name)) = MODEL_NAME_OVERRIDES
        .iter()
        .find(|(id, _)| *id == normalized)
    {
        return name.to_string();
    }
    if let Some(captures) = CLAUDE_MODEL.captures(&normalized) {
        return format!(
            "Claude {} {}",
            capitalize(&captures[1]),
            format_version(&captures[2])
        );
    }
    if let Some(captures) = GPT_MODEL.captures(&normalized) {
        return format!("GPT {}", format_version(&captures[1]));
    }
    for (family, display) in [
        ("gemini", "Gemini"),
        ("mistral", "Mistral"),
        ("voxtral", "Voxtral"),
        ("ministral", "Ministral"),
        ("magistral", "Magistral"),
        ("kimi", "Kimi"),
    ] {
        if let Some(rest) = normalized.strip_prefix(&format!("{family}-"))
            && !rest.is_empty()
        {
            return format!("{display} {}", format_version(rest));
        }
    }
    normalized
        .split(['-', '_'])
        .map(format_token)
        .collect::<Vec<_>>()
        .join(" ")
}

/// `formatVersion`: dashes to spaces, `5 6` to `5.6`, `latest` dropped.
fn format_version(value: &str) -> String {
    let spaced = value.replace('-', " ");
    let dotted = VERSION_PAIR.replace_all(&spaced, "$1.$2").to_string();
    dotted
        .split(' ')
        .filter(|part| *part != "latest")
        .map(format_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_token(token: &str) -> String {
    if token.is_empty() {
        return String::new();
    }
    if ["api", "oss", "vl", "llm", "ai"].contains(&token) {
        return token.to_uppercase();
    }
    // `\b[a-z]` → the first letter after each word boundary.
    let mut out = String::with_capacity(token.len());
    let mut at_boundary = true;
    for character in token.chars() {
        if at_boundary && character.is_ascii_lowercase() {
            out.push(character.to_ascii_uppercase());
        } else {
            out.push(character);
        }
        at_boundary = !character.is_alphanumeric() && character != '_';
    }
    out
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// `REASONING_EFFORTS` / `normalizeReasoningEffort`
pub fn normalize_reasoning_effort(value: &str) -> &'static str {
    match value {
        "low" => "low",
        "medium" => "medium",
        "high" => "high",
        _ => "default",
    }
}

/// `supportsReasoningEffort`: Anarlog Pro picks its own model settings, and
/// Apple Foundation has no reasoning knob to turn.
pub fn supports_reasoning_effort(provider_id: &str) -> bool {
    provider_id != "anarlog" && provider_id != "apple_foundation"
}

fn partition<T>(
    items: &[T],
    should_ignore: impl Fn(&T) -> Vec<IgnoreReason>,
    extract: impl Fn(&T) -> String,
) -> (Vec<String>, Vec<IgnoredModel>) {
    let mut models = Vec::new();
    let mut ignored = Vec::new();
    for item in items {
        let reasons = should_ignore(item);
        let id = extract(item);
        if reasons.is_empty() {
            models.push(id);
        } else {
            ignored.push(IgnoredModel { id, reasons });
        }
    }
    (models, ignored)
}

fn metadata_map<T>(
    items: &[T],
    extract: impl Fn(&T) -> String,
    modalities: impl Fn(&T) -> Vec<InputModality>,
) -> BTreeMap<String, Vec<InputModality>> {
    items
        .iter()
        .filter_map(|item| {
            let modalities = modalities(item);
            (!modalities.is_empty()).then(|| (extract(item), modalities))
        })
        .collect()
}

const TEXT: &[InputModality] = &[InputModality::Text];
const TEXT_IMAGE: &[InputModality] = &[InputModality::Text, InputModality::Image];

#[derive(Deserialize)]
struct IdModel {
    id: String,
}

#[derive(Deserialize)]
struct IdList {
    data: Vec<IdModel>,
}

/// `processGenericModels` (also the OpenAI shape, whose metadata is
/// text+image).
pub fn process_generic_models(
    ids: &[String],
    filter_date_snapshots: bool,
    modalities: &'static [InputModality],
) -> ListModelsResult {
    let (models, ignored) = partition(
        ids,
        |id| {
            let mut reasons = Vec::new();
            if should_ignore_common_keywords(id) {
                reasons.push(IgnoreReason::CommonKeyword);
            }
            if is_non_chat_model(id) {
                reasons.push(IgnoreReason::NotChatModel);
            }
            if is_old_model(id) {
                reasons.push(IgnoreReason::OldModel);
            }
            if filter_date_snapshots && is_date_snapshot(id) {
                reasons.push(IgnoreReason::DateSnapshot);
            }
            reasons
        },
        |id| id.clone(),
    );
    ListModelsResult {
        models: sort_models_by_recency(&models),
        ignored,
        metadata: metadata_map(ids, |id| id.clone(), |_| modalities.to_vec()),
    }
}

#[derive(Deserialize)]
struct AnthropicList {
    data: Vec<IdModel>,
}

fn process_anthropic_models(ids: &[String]) -> ListModelsResult {
    let (models, ignored) = partition(
        ids,
        |id| {
            let mut reasons = Vec::new();
            if should_ignore_common_keywords(id) {
                reasons.push(IgnoreReason::CommonKeyword);
            }
            if is_old_model(id) {
                reasons.push(IgnoreReason::OldModel);
            }
            reasons
        },
        |id| id.clone(),
    );
    ListModelsResult {
        models: sort_models_by_recency(&models),
        ignored,
        metadata: metadata_map(ids, |id| id.clone(), |_| TEXT_IMAGE.to_vec()),
    }
}

#[derive(Deserialize)]
struct GoogleModel {
    name: String,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct GoogleList {
    models: Vec<GoogleModel>,
}

fn process_google_models(models: &[GoogleModel]) -> ListModelsResult {
    let extract = |model: &GoogleModel| model.name.trim_start_matches("models/").to_string();
    let (kept, ignored) = partition(
        models,
        |model| {
            let id = extract(model);
            let mut reasons = Vec::new();
            if should_ignore_common_keywords(&model.name) {
                reasons.push(IgnoreReason::CommonKeyword);
            }
            if is_non_chat_model(&id) {
                reasons.push(IgnoreReason::NotChatModel);
            }
            if is_old_model(&id) {
                reasons.push(IgnoreReason::OldModel);
            }
            let supports_generation = model
                .supported_generation_methods
                .as_ref()
                .is_none_or(|methods| methods.iter().any(|m| m == "generateContent"));
            if !supports_generation {
                reasons.push(IgnoreReason::NoCompletion);
            }
            if is_date_snapshot(&id) {
                reasons.push(IgnoreReason::DateSnapshot);
            }
            reasons
        },
        extract,
    );
    ListModelsResult {
        models: sort_models_by_recency(&kept),
        ignored,
        metadata: metadata_map(models, extract, |model| {
            if extract(model).to_lowercase().contains("gemini") {
                TEXT_IMAGE.to_vec()
            } else {
                TEXT.to_vec()
            }
        }),
    }
}

#[derive(Deserialize)]
struct MistralCapabilities {
    completion_chat: bool,
    vision: bool,
}

#[derive(Deserialize)]
struct MistralModel {
    id: String,
    capabilities: MistralCapabilities,
}

#[derive(Deserialize)]
struct MistralList {
    data: Vec<MistralModel>,
}

fn process_mistral_models(models: &[MistralModel]) -> ListModelsResult {
    let (kept, ignored) = partition(
        models,
        |model| {
            let mut reasons = Vec::new();
            if should_ignore_common_keywords(&model.id) {
                reasons.push(IgnoreReason::CommonKeyword);
            }
            if !model.capabilities.completion_chat {
                reasons.push(IgnoreReason::NoCompletion);
            }
            if is_old_model(&model.id) {
                reasons.push(IgnoreReason::OldModel);
            }
            reasons
        },
        |model| model.id.clone(),
    );
    ListModelsResult {
        models: sort_models_by_recency(&kept),
        ignored,
        metadata: metadata_map(
            models,
            |model| model.id.clone(),
            |model| {
                if model.capabilities.vision {
                    TEXT_IMAGE.to_vec()
                } else {
                    TEXT.to_vec()
                }
            },
        ),
    }
}

#[derive(Deserialize)]
struct OpenRouterArchitecture {
    input_modalities: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct OpenRouterModel {
    id: String,
    supported_parameters: Option<Vec<String>>,
    architecture: Option<OpenRouterArchitecture>,
}

#[derive(Deserialize)]
struct OpenRouterList {
    data: Vec<OpenRouterModel>,
}

fn openrouter_input_modalities(model: &OpenRouterModel) -> Option<&Vec<String>> {
    model
        .architecture
        .as_ref()
        .and_then(|architecture| architecture.input_modalities.as_ref())
}

fn process_openrouter_models(models: &[OpenRouterModel]) -> ListModelsResult {
    let input_modalities = openrouter_input_modalities;
    let (kept, ignored) = partition(
        models,
        |model| {
            let mut reasons = Vec::new();
            if should_ignore_common_keywords(&model.id) {
                reasons.push(IgnoreReason::CommonKeyword);
            }
            if is_non_chat_model(&model.id) {
                reasons.push(IgnoreReason::NotChatModel);
            }
            let supports_text = input_modalities(model)
                .is_none_or(|modalities| modalities.iter().any(|m| m == "text"));
            if !supports_text {
                reasons.push(IgnoreReason::NoTextInput);
            }
            let supports_tools = model
                .supported_parameters
                .as_ref()
                .is_none_or(|parameters| {
                    ["tools", "tool_choice"]
                        .iter()
                        .all(|parameter| parameters.iter().any(|p| p == parameter))
                });
            if !supports_tools {
                reasons.push(IgnoreReason::NoTool);
            }
            if is_old_model(&model.id) {
                reasons.push(IgnoreReason::OldModel);
            }
            reasons
        },
        |model| model.id.clone(),
    );
    ListModelsResult {
        models: sort_models_by_recency(&kept),
        ignored,
        metadata: metadata_map(
            models,
            |model| model.id.clone(),
            |model| {
                let modalities = input_modalities(model).cloned().unwrap_or_default();
                let mut out = Vec::new();
                if modalities.iter().any(|m| m == "text") {
                    out.push(InputModality::Text);
                }
                if modalities.iter().any(|m| m == "image") {
                    out.push(InputModality::Image);
                }
                out
            },
        ),
    }
}

#[derive(Deserialize)]
struct AzureCapabilities {
    chat_completion: Option<bool>,
    completion: Option<bool>,
}

#[derive(Deserialize)]
struct AzureOpenAiModel {
    id: String,
    capabilities: Option<AzureCapabilities>,
}

#[derive(Deserialize)]
struct AzureOpenAiList {
    data: Vec<AzureOpenAiModel>,
}

fn process_azure_openai_models(models: &[AzureOpenAiModel]) -> ListModelsResult {
    let (kept, ignored) = partition(
        models,
        |model| {
            let mut reasons = Vec::new();
            if should_ignore_common_keywords(&model.id) {
                reasons.push(IgnoreReason::CommonKeyword);
            }
            if is_non_chat_model(&model.id) {
                reasons.push(IgnoreReason::NotChatModel);
            }
            if is_old_model(&model.id) {
                reasons.push(IgnoreReason::OldModel);
            }
            if is_date_snapshot(&model.id) {
                reasons.push(IgnoreReason::DateSnapshot);
            }
            if model.capabilities.as_ref().is_some_and(|capabilities| {
                capabilities.chat_completion == Some(false)
                    && capabilities.completion == Some(false)
            }) {
                reasons.push(IgnoreReason::NotChatModel);
            }
            reasons
        },
        |model| model.id.clone(),
    );
    ListModelsResult {
        models: sort_models_by_recency(&kept),
        ignored,
        metadata: metadata_map(models, |model| model.id.clone(), |_| TEXT_IMAGE.to_vec()),
    }
}

fn process_azure_ai_models(ids: &[String]) -> ListModelsResult {
    let (kept, ignored) = partition(
        ids,
        |id| {
            let mut reasons = Vec::new();
            if should_ignore_common_keywords(id) {
                reasons.push(IgnoreReason::CommonKeyword);
            }
            if is_old_model(id) {
                reasons.push(IgnoreReason::OldModel);
            }
            reasons
        },
        |id| id.clone(),
    );
    ListModelsResult {
        models: sort_models_by_recency(&kept),
        ignored,
        metadata: metadata_map(ids, |id| id.clone(), |_| TEXT_IMAGE.to_vec()),
    }
}

/// `processUnslothModels`: only clearly non-chat entries drop.
pub fn process_unsloth_models(ids: &[String]) -> ListModelsResult {
    let (mut models, ignored) = partition(
        ids,
        |id| {
            if should_ignore_common_keywords(id) {
                vec![IgnoreReason::CommonKeyword]
            } else {
                Vec::new()
            }
        },
        |id| id.clone(),
    );
    models.sort();
    ListModelsResult {
        models,
        ignored,
        metadata: metadata_map(ids, |id| id.clone(), |_| TEXT.to_vec()),
    }
}

#[derive(Deserialize)]
struct LmStudioCapabilities {
    trained_for_tool_use: Option<bool>,
    vision: Option<bool>,
}

#[derive(Deserialize)]
pub struct LmStudioModel {
    #[serde(rename = "type")]
    kind: String,
    key: String,
    loaded_instances: Vec<serde_json::Value>,
    max_context_length: f64,
    capabilities: Option<LmStudioCapabilities>,
}

#[derive(Deserialize)]
struct LmStudioList {
    models: Vec<LmStudioModel>,
}

/// `processLMStudioModels`
pub fn process_lmstudio_models(downloaded: &[LmStudioModel]) -> ListModelsResult {
    let mut models = Vec::new();
    let mut ignored = Vec::new();
    let mut metadata = BTreeMap::new();
    for model in downloaded {
        let mut reasons = Vec::new();
        if model.kind != "llm" {
            reasons.push(IgnoreReason::NotLlm);
        } else {
            if model
                .capabilities
                .as_ref()
                .is_some_and(|c| c.trained_for_tool_use == Some(false))
            {
                reasons.push(IgnoreReason::NoTool);
            }
            if model.max_context_length <= 15_000.0 {
                reasons.push(IgnoreReason::ContextTooSmall);
            }
        }
        if reasons.is_empty() {
            models.push(model.key.clone());
            metadata.insert(
                model.key.clone(),
                if model
                    .capabilities
                    .as_ref()
                    .is_some_and(|c| c.vision == Some(true))
                {
                    TEXT_IMAGE.to_vec()
                } else {
                    TEXT.to_vec()
                },
            );
        } else {
            ignored.push(IgnoredModel {
                id: model.key.clone(),
                reasons,
            });
        }
    }
    let loaded: Vec<&str> = downloaded
        .iter()
        .filter(|model| !model.loaded_instances.is_empty())
        .map(|model| model.key.as_str())
        .collect();
    // A stable sort keeps the catalogue order within each group.
    models.sort_by_key(|key| !loaded.contains(&key.as_str()));
    ListModelsResult {
        models,
        ignored,
        metadata,
    }
}

/// `getLMStudioNativeModelsUrl`
pub fn lmstudio_native_models_url(base_url: &str) -> Option<String> {
    let mut url = url::Url::parse(base_url).ok()?;
    let path = url.path().trim_end_matches('/').to_string();
    let next = if path.ends_with("/api/v1") {
        format!("{path}/models")
    } else if let Some(prefix) = path.strip_suffix("/v1") {
        format!("{prefix}/api/v1/models")
    } else {
        format!("{path}/api/v1/models")
    };
    url.set_path(&next);
    Some(url.to_string())
}

/// `CLOUDFLARE_WORKERS_AI_MODELS`
pub const CLOUDFLARE_WORKERS_AI_MODELS: &[&str] = &[
    "@cf/moonshotai/kimi-k2.7-code",
    "@cf/zai-org/glm-5.2",
    "@cf/moonshotai/kimi-k2.6",
    "@cf/zai-org/glm-4.7-flash",
    "@cf/openai/gpt-oss-120b",
    "@cf/meta/llama-4-scout-17b-16e-instruct",
    "@cf/google/gemma-4-26b-a4b-it",
    "@cf/nvidia/nemotron-3-120b-a12b",
    "@cf/openai/gpt-oss-20b",
    "@cf/qwen/qwen3-30b-a3b-fp8",
    "@cf/mistralai/mistral-small-3.1-24b-instruct",
    "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
];

pub const CLOUDFLARE_VISION_MODELS: &[&str] = &[
    "@cf/moonshotai/kimi-k2.7-code",
    "@cf/moonshotai/kimi-k2.6",
    "@cf/meta/llama-4-scout-17b-16e-instruct",
    "@cf/google/gemma-4-26b-a4b-it",
];

/// `GOOGLE_VERTEX_AI_MODELS`
pub const GOOGLE_VERTEX_AI_MODELS: &[&str] = &[
    "google/gemini-3.8-flash",
    "google/gemini-3.7-flash",
    "google/gemini-3.6-flash",
    "google/gemini-3.5-flash-lite",
    "google/gemini-3.1-pro-preview",
    "google/gemini-3.5-flash",
    "google/gemini-3.1-flash-lite",
];

fn static_result(
    models: &[&str],
    modalities: impl Fn(&str) -> Vec<InputModality>,
) -> ListModelsResult {
    ListModelsResult {
        models: models.iter().map(|m| m.to_string()).collect(),
        ignored: Vec::new(),
        metadata: models
            .iter()
            .map(|model| (model.to_string(), modalities(model)))
            .collect(),
    }
}

/// `createStaticCloudflareWorkersAIModelResult`
pub fn cloudflare_workers_ai_models() -> ListModelsResult {
    static_result(CLOUDFLARE_WORKERS_AI_MODELS, |model| {
        if CLOUDFLARE_VISION_MODELS.contains(&model) {
            TEXT_IMAGE.to_vec()
        } else {
            TEXT.to_vec()
        }
    })
}

async fn fetch_json(
    client: &reqwest::Client,
    url: &str,
    headers: &[(&str, String)],
) -> anyhow::Result<serde_json::Value> {
    let mut request = client.get(url);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let response = request.send().await?;
    if !response.status().is_success() {
        anyhow::bail!("HTTP {}", response.status());
    }
    if response
        .content_length()
        .is_some_and(|length| length as usize > MAX_MODEL_RESPONSE_BYTES)
    {
        anyhow::bail!("Response body exceeds {MAX_MODEL_RESPONSE_BYTES} bytes");
    }
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_MODEL_RESPONSE_BYTES {
        anyhow::bail!("Response body exceeds {MAX_MODEL_RESPONSE_BYTES} bytes");
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn ids_of(value: serde_json::Value) -> anyhow::Result<Vec<String>> {
    let list: IdList = serde_json::from_value(value)?;
    Ok(list.data.into_iter().map(|model| model.id).collect())
}

fn bearer(api_key: &str) -> Vec<(&'static str, String)> {
    vec![("Authorization", format!("Bearer {api_key}"))]
}

/// A trimmed key only (`getLMStudioHeaders` / `getUnslothHeaders`).
fn optional_bearer(api_key: &str) -> Vec<(&'static str, String)> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        bearer(trimmed)
    }
}

async fn list_ollama(client: &reqwest::Client, base_url: &str) -> anyhow::Result<ListModelsResult> {
    #[derive(Deserialize)]
    struct Named {
        name: String,
    }
    #[derive(Deserialize)]
    struct Tags {
        #[serde(default)]
        models: Vec<Named>,
    }
    #[derive(Deserialize)]
    struct Show {
        #[serde(default)]
        capabilities: Vec<String>,
    }
    // `createOllamaClient`: the host is the base URL without `/v1`.
    let re = Regex::new(r"/v1/?$").expect("valid pattern");
    let host = re.replace(base_url, "").to_string();
    let host = host.trim_end_matches('/');
    let tags: Tags =
        serde_json::from_value(fetch_json(client, &format!("{host}/api/tags"), &[]).await?)?;
    let running: Vec<String> = match fetch_json(client, &format!("{host}/api/ps"), &[]).await {
        Ok(value) => serde_json::from_value::<Tags>(value)
            .map(|ps| ps.models.into_iter().map(|m| m.name).collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    let mut details = Vec::new();
    for model in &tags.models {
        let capabilities = match client
            .post(format!("{host}/api/show"))
            .json(&serde_json::json!({ "model": model.name }))
            .send()
            .await
        {
            Ok(response) => response
                .json::<Show>()
                .await
                .map(|show| show.capabilities)
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        details.push((
            model.name.clone(),
            capabilities,
            running.contains(&model.name),
        ));
    }
    Ok(summarize_ollama_details(&details))
}

/// `summarizeOllamaDetails`: completion + tools keep a model, running ones
/// first.
pub fn summarize_ollama_details(details: &[(String, Vec<String>, bool)]) -> ListModelsResult {
    let mut supported: Vec<(String, bool)> = Vec::new();
    let mut ignored = Vec::new();
    let mut metadata = BTreeMap::new();
    for (name, capabilities, running) in details {
        let has_completion = capabilities.iter().any(|c| c == "completion");
        let has_tools = capabilities.iter().any(|c| c == "tools");
        if has_completion && has_tools {
            supported.push((name.clone(), *running));
            metadata.insert(name.clone(), TEXT.to_vec());
        } else {
            let mut reasons = Vec::new();
            if !has_completion {
                reasons.push(IgnoreReason::NoCompletion);
            }
            if !has_tools {
                reasons.push(IgnoreReason::NoTool);
            }
            ignored.push(IgnoredModel {
                id: name.clone(),
                reasons,
            });
        }
    }
    supported
        .sort_by(|(a, a_running), (b, b_running)| b_running.cmp(a_running).then_with(|| a.cmp(b)));
    ListModelsResult {
        models: supported.into_iter().map(|(name, _)| name).collect(),
        ignored,
        metadata,
    }
}

/// `getLlmProviderStatus`'s `listModels` for a configured provider, with
/// `removeNonStreamingModels` applied; every failure yields the empty result
/// like `Effect.catchAll(() => DEFAULT_RESULT)`.
pub async fn list_llm_models(provider_id: &str, base_url: &str, api_key: &str) -> ListModelsResult {
    let result = list_llm_models_inner(provider_id, base_url, api_key)
        .await
        .unwrap_or_else(|error| {
            tracing::debug!(%error, provider = provider_id, "model listing failed");
            ListModelsResult::default()
        });
    remove_non_streaming_models(result)
}

async fn list_llm_models_inner(
    provider_id: &str,
    base_url: &str,
    api_key: &str,
) -> anyhow::Result<ListModelsResult> {
    match provider_id {
        "anarlog" => {
            return Ok(static_result(&["Auto"], |_| TEXT_IMAGE.to_vec()));
        }
        "google_vertex_ai" => {
            return Ok(static_result(GOOGLE_VERTEX_AI_MODELS, |_| {
                TEXT_IMAGE.to_vec()
            }));
        }
        "cloudflare_workers_ai" => {
            return Ok(if base_url.is_empty() {
                ListModelsResult::default()
            } else {
                cloudflare_workers_ai_models()
            });
        }
        _ => {}
    }
    if base_url.is_empty() {
        return Ok(ListModelsResult::default());
    }
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    let base = base_url.trim_end_matches('/');
    let result = match provider_id {
        "openai" => process_generic_models(
            &ids_of(fetch_json(&client, &format!("{base_url}/models"), &bearer(api_key)).await?)?,
            true,
            TEXT_IMAGE,
        ),
        "cohere" => process_generic_models(
            &ids_of(fetch_json(&client, &format!("{base_url}/models"), &bearer(api_key)).await?)?,
            false,
            TEXT,
        ),
        "anthropic" => {
            let list: AnthropicList = serde_json::from_value(
                fetch_json(
                    &client,
                    &format!("{base_url}/models"),
                    &[
                        ("anthropic-version", "2023-06-01".to_string()),
                        (
                            "anthropic-dangerous-direct-browser-access",
                            "true".to_string(),
                        ),
                        ("x-api-key", api_key.to_string()),
                    ],
                )
                .await?,
            )?;
            let ids: Vec<String> = list.data.into_iter().map(|model| model.id).collect();
            process_anthropic_models(&ids)
        }
        "openrouter" => {
            let list: OpenRouterList = serde_json::from_value(
                fetch_json(&client, &format!("{base_url}/models"), &bearer(api_key)).await?,
            )?;
            process_openrouter_models(&list.data)
        }
        "google_generative_ai" => {
            let list: GoogleList = serde_json::from_value(
                fetch_json(
                    &client,
                    &format!("{base_url}/models"),
                    &[("x-goog-api-key", api_key.to_string())],
                )
                .await?,
            )?;
            process_google_models(&list.models)
        }
        "mistral" => {
            let list: MistralList = serde_json::from_value(
                fetch_json(&client, &format!("{base_url}/models"), &bearer(api_key)).await?,
            )?;
            process_mistral_models(&list.data)
        }
        "azure_openai" => {
            let list: AzureOpenAiList = serde_json::from_value(
                fetch_json(
                    &client,
                    &format!("{base}/openai/models?api-version=2024-10-21"),
                    &[("api-key", api_key.to_string())],
                )
                .await?,
            )?;
            process_azure_openai_models(&list.data)
        }
        "azure_ai" => process_azure_ai_models(&ids_of(
            fetch_json(
                &client,
                &format!("{base}/models"),
                &[("api-key", api_key.to_string())],
            )
            .await?,
        )?),
        "ollama" => list_ollama(&client, base_url).await?,
        "lmstudio" => {
            let native = match lmstudio_native_models_url(base_url) {
                Some(url) => fetch_json(&client, &url, &optional_bearer(api_key))
                    .await
                    .and_then(|value| Ok(serde_json::from_value::<LmStudioList>(value)?)),
                None => Err(anyhow::anyhow!("invalid base url")),
            };
            match native {
                Ok(list) => process_lmstudio_models(&list.models),
                // `catchAll(() => listGenericModels(...))`
                Err(_) => process_generic_models(
                    &ids_of(
                        fetch_json(&client, &format!("{base_url}/models"), &bearer(api_key))
                            .await?,
                    )?,
                    true,
                    TEXT,
                ),
            }
        }
        "unsloth" => process_unsloth_models(&ids_of(
            fetch_json(
                &client,
                &format!("{base}/models"),
                &optional_bearer(api_key),
            )
            .await?,
        )?),
        // `apple_foundation` is macOS-only and the subscription providers need
        // a signed-in account; everything else is OpenAI-compatible.
        _ => process_generic_models(
            &ids_of(fetch_json(&client, &format!("{base_url}/models"), &bearer(api_key)).await?)?,
            true,
            TEXT,
        ),
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn display_names_follow_model_display() {
        // `model-display.test.ts`
        assert_eq!(
            display_llm_model_id("openrouter", "~anthropic/claude-sonnet-latest"),
            "Claude Sonnet 5"
        );
        assert_eq!(display_llm_model_id("openai", "chat-latest"), "Chat Latest");
        assert_eq!(
            display_llm_model_id("openrouter", "anthropic/claude-haiku-4-5-20251001"),
            "Claude Haiku 4.5"
        );
        assert_eq!(
            display_llm_model_id("openrouter", "mistralai/mistral-large-2512"),
            "Mistral Large"
        );
        assert_eq!(
            display_llm_model_id("amazon_bedrock", "anthropic.claude-opus-5"),
            "Claude Opus 5"
        );
        assert_eq!(
            display_llm_model_id("cohere", "command-a-plus-05-2026"),
            "Command A Plus"
        );
        assert_eq!(display_llm_model_id("openai", "gpt-5.5"), "GPT 5.5");
        assert_eq!(
            display_llm_model_id("google_generative_ai", "gemini-3.5-flash"),
            "Gemini 3.5 Flash"
        );
        assert_eq!(display_llm_model_id("anarlog", "Auto"), "Pro (Cloud)");
        assert_eq!(
            display_llm_model_id("openai", "gpt-oss-120b"),
            "GPT OSS 120b"
        );
        assert_eq!(
            display_llm_model_id("custom", "llama_3_local"),
            "Llama 3 Local"
        );
    }

    #[test]
    fn model_names_drop_paths_and_dotted_providers() {
        assert_eq!(model_name("openai/gpt-5.6"), "gpt-5.6");
        assert_eq!(model_name("anthropic.claude-opus-5"), "claude-opus-5");
        assert_eq!(model_name("~GPT-5.5"), "gpt-5.5");
        assert_eq!(model_name("google.gemma-3-27b-it"), "gemma-3-27b-it");
        assert_eq!(model_name("plain-model"), "plain-model");
    }

    #[test]
    fn generic_processing_matches_the_frontend_fixtures() {
        // `keeps Cohere model versions while still filtering non-chat models`
        let result =
            process_generic_models(&ids(&["command-a-plus-05-2026", "embed-v4.0"]), false, TEXT);
        assert_eq!(result.models, ids(&["command-a-plus-05-2026"]));
        assert_eq!(
            result.ignored,
            vec![IgnoredModel {
                id: "embed-v4.0".into(),
                reasons: vec![IgnoreReason::CommonKeyword]
            }]
        );
        // `filters date snapshots by default for generic providers`
        let result = process_generic_models(&ids(&["model-05-2026"]), true, TEXT);
        assert!(result.models.is_empty());
        assert_eq!(result.ignored[0].reasons, vec![IgnoreReason::DateSnapshot]);
        // `filters dotted Bedrock model IDs by their model name`
        let result = process_generic_models(
            &ids(&[
                "anthropic.claude-opus-4.7",
                "google.gemma-3-27b-it",
                "anthropic.claude-opus-5",
            ]),
            true,
            TEXT,
        );
        assert_eq!(
            result.models,
            ids(&["anthropic.claude-opus-5", "google.gemma-3-27b-it"])
        );
        assert_eq!(
            result.ignored,
            vec![IgnoredModel {
                id: "anthropic.claude-opus-4.7".into(),
                reasons: vec![IgnoreReason::OldModel]
            }]
        );
    }

    #[test]
    fn ignore_rules_follow_list_common() {
        assert!(should_ignore_common_keywords("text-embedding-3-large"));
        assert!(should_ignore_common_keywords("gpt-4o-realtime"));
        assert!(!should_ignore_common_keywords("gpt-5.6"));
        assert!(is_date_snapshot("gpt-5.6-2026-03-01"));
        assert!(is_date_snapshot("claude-opus-5-20260301"));
        assert!(is_date_snapshot("mistral-large-2025"));
        assert!(!is_date_snapshot("mistral-large-2512"));
        assert!(!is_date_snapshot("gpt-5.6"));
        assert!(is_non_chat_model("o3-mini"));
        assert!(is_non_chat_model("gpt-4o-mini"));
        assert!(is_non_chat_model("ft:gpt-5.6:org::abc"));
        assert!(is_non_chat_model("xai/grok-code-fast-2"));
        assert!(!is_non_chat_model("gpt-5.6"));
        assert!(is_non_streaming_model("gpt-5.6-pro"));
        assert!(!is_non_streaming_model("gpt-5.6"));
        assert!(is_old_model("gpt-4.1"));
        assert!(is_old_model("gpt-5-mini"));
        assert!(is_old_model("gpt-5.2"));
        assert!(!is_old_model("gpt-5.4"));
        assert!(is_old_model("claude-opus-4.7"));
        assert!(!is_old_model("claude-opus-4.8"));
        assert!(is_old_model("claude-haiku-4"));
        assert!(!is_old_model("claude-haiku-4.5"));
        assert!(is_old_model("gemini-2.5-pro"));
        assert!(is_old_model("gemini-3-pro-preview"));
        assert!(!is_old_model("gemini-3.8-flash"));
        assert!(is_old_model("grok-4"));
        assert!(!is_old_model("grok-4.6"));
        assert!(is_old_model("kimi-k2.5"));
        assert!(!is_old_model("kimi-k3"));
    }

    #[test]
    fn recency_sort_puts_priority_patterns_first() {
        let sorted = sort_models_by_recency(&ids(&[
            "zeta",
            "gpt-5.5",
            "claude-opus-5",
            "alpha",
            "openai/gpt-5.6",
        ]));
        assert_eq!(
            sorted,
            ids(&[
                "openai/gpt-5.6",
                "gpt-5.5",
                "claude-opus-5",
                "alpha",
                "zeta"
            ])
        );
    }

    #[test]
    fn non_streaming_models_are_removed_everywhere() {
        let result = remove_non_streaming_models(ListModelsResult {
            models: ids(&["gpt-5.6-pro", "gpt-5.6"]),
            ignored: vec![IgnoredModel {
                id: "gpt-5.5-pro".into(),
                reasons: vec![IgnoreReason::OldModel],
            }],
            metadata: [("gpt-5.6-pro".to_string(), TEXT.to_vec())]
                .into_iter()
                .collect(),
        });
        assert_eq!(result.models, ids(&["gpt-5.6"]));
        assert!(result.ignored.is_empty());
        assert!(result.metadata.is_empty());
    }

    #[test]
    fn openrouter_requires_text_and_tools() {
        let models: Vec<OpenRouterModel> = serde_json::from_value(serde_json::json!([
            { "id": "openai/gpt-5.6", "supported_parameters": ["tools", "tool_choice"],
              "architecture": { "input_modalities": ["text", "image"] } },
            { "id": "vendor/no-tools", "supported_parameters": ["temperature"] },
            { "id": "vendor/pics-only", "architecture": { "input_modalities": ["image"] } },
            { "id": "vendor/plain" }
        ]))
        .unwrap();
        let result = process_openrouter_models(&models);
        assert_eq!(result.models, ids(&["openai/gpt-5.6", "vendor/plain"]));
        assert_eq!(result.ignored[0].reasons, vec![IgnoreReason::NoTool]);
        assert_eq!(result.ignored[1].reasons, vec![IgnoreReason::NoTextInput]);
        assert_eq!(
            result.metadata["openai/gpt-5.6"],
            vec![InputModality::Text, InputModality::Image]
        );
        assert!(!result.metadata.contains_key("vendor/plain"));
    }

    #[test]
    fn lmstudio_filters_and_orders_loaded_models_first() {
        // `list-lmstudio.test.ts`
        let list: LmStudioList = serde_json::from_value(serde_json::json!({ "models": [
            { "type": "llm", "key": "unloaded", "loaded_instances": [], "max_context_length": 32000,
              "capabilities": { "trained_for_tool_use": true, "vision": false } },
            { "type": "llm", "key": "loaded", "loaded_instances": [{}], "max_context_length": 32000,
              "capabilities": { "trained_for_tool_use": true, "vision": true } },
            { "type": "embedding", "key": "embed", "loaded_instances": [], "max_context_length": 512 },
            { "type": "llm", "key": "tiny", "loaded_instances": [], "max_context_length": 4096,
              "capabilities": { "trained_for_tool_use": false } }
        ] }))
        .unwrap();
        let result = process_lmstudio_models(&list.models);
        assert_eq!(result.models, ids(&["loaded", "unloaded"]));
        assert_eq!(result.ignored[0].reasons, vec![IgnoreReason::NotLlm]);
        assert_eq!(
            result.ignored[1].reasons,
            vec![IgnoreReason::NoTool, IgnoreReason::ContextTooSmall]
        );
        assert_eq!(
            result.metadata["loaded"],
            vec![InputModality::Text, InputModality::Image]
        );
        assert_eq!(
            lmstudio_native_models_url("http://localhost:1234/v1").unwrap(),
            "http://localhost:1234/api/v1/models"
        );
        assert_eq!(
            lmstudio_native_models_url("http://localhost:1234/api/v1/").unwrap(),
            "http://localhost:1234/api/v1/models"
        );
        assert_eq!(
            lmstudio_native_models_url("http://localhost:1234").unwrap(),
            "http://localhost:1234/api/v1/models"
        );
    }

    #[test]
    fn ollama_keeps_tool_capable_models_running_first() {
        let result = summarize_ollama_details(&[
            (
                "zephyr".into(),
                vec!["completion".into(), "tools".into()],
                false,
            ),
            (
                "llama".into(),
                vec!["completion".into(), "tools".into()],
                true,
            ),
            ("embed".into(), vec!["embedding".into()], false),
            ("chat-only".into(), vec!["completion".into()], false),
        ]);
        assert_eq!(result.models, ids(&["llama", "zephyr"]));
        assert_eq!(
            result.ignored[0].reasons,
            vec![IgnoreReason::NoCompletion, IgnoreReason::NoTool]
        );
        assert_eq!(result.ignored[1].reasons, vec![IgnoreReason::NoTool]);
    }

    #[test]
    fn unsloth_only_drops_common_keywords() {
        let result = process_unsloth_models(&ids(&["zeta-gguf", "alpha-gguf", "embed-x"]));
        assert_eq!(result.models, ids(&["alpha-gguf", "zeta-gguf"]));
        assert_eq!(result.ignored[0].reasons, vec![IgnoreReason::CommonKeyword]);
    }

    /// `list-openai.test.ts`: `lists Meta Muse Spark models with the current release first`.
    #[test]
    fn lists_meta_muse_spark_models_with_the_current_release_first() {
        let result = process_generic_models(
            &ids(&[
                "muse-spark-1.3-contributor",
                "muse-voice-transcribe-1.0",
                "muse-spark-1.3",
                "muse-image-1.0",
                "muse-spark-1.2-contributor",
                "muse-spark-1.2",
                "muse-spark-1.1",
            ]),
            true,
            &[InputModality::Text],
        );
        assert_eq!(
            result.models,
            ids(&[
                "muse-spark-1.3",
                "muse-spark-1.1",
                "muse-spark-1.2",
                "muse-spark-1.2-contributor",
                "muse-spark-1.3-contributor",
            ])
        );
        assert_eq!(
            result
                .ignored
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            vec!["muse-voice-transcribe-1.0", "muse-image-1.0"]
        );
    }

    #[test]
    fn static_catalogues_carry_vision_metadata() {
        let cloudflare = cloudflare_workers_ai_models();
        assert_eq!(cloudflare.models.len(), CLOUDFLARE_WORKERS_AI_MODELS.len());
        assert_eq!(
            cloudflare.metadata["@cf/google/gemma-4-26b-a4b-it"],
            vec![InputModality::Text, InputModality::Image]
        );
        assert_eq!(
            cloudflare.metadata["@cf/openai/gpt-oss-20b"],
            vec![InputModality::Text]
        );
    }
}
