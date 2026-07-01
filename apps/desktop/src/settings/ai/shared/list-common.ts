import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { Effect } from "effect";

export type ModelIgnoreReason =
  | "common_keyword"
  | "old_model"
  | "date_snapshot"
  | "no_tool"
  | "no_text_input"
  | "no_completion"
  | "not_llm"
  | "not_chat_model"
  | "context_too_small";

export type IgnoredModel = { id: string; reasons: ModelIgnoreReason[] };

export type InputModality = "image" | "text";

export type ModelMetadata = {
  input_modalities?: InputModality[];
};

export type ListModelsResult = {
  models: string[];
  ignored: IgnoredModel[];
  metadata: Record<string, ModelMetadata>;
};

export const DEFAULT_RESULT: ListModelsResult = {
  models: [],
  ignored: [],
  metadata: {},
};
export const REQUEST_TIMEOUT = "5 seconds";

const commonIgnoreKeywords = [
  "embed",
  "sora",
  "tts",
  "whisper",
  "dall-e",
  "audio",
  "image",
  "computer",
  "robotics",
  "realtime",
  "moderation",
  "codex",
  "transcribe",
  "search-api",
] as const;

const modelPriorityPatterns = [
  /(?:^|\/)gpt-5\.5-pro$/,
  /(?:^|\/)gpt-5\.5$/,
  /(?:^|\/)(?:chat-latest|gpt-chat-latest)$/,
  /(?:^|\/)claude-sonnet-(?:5|latest)$/,
  /(?:^|\/)gpt-5\.4-pro$/,
  /(?:^|\/)gpt-5\.4$/,
  /(?:^|\/)gpt-5\.4-mini$/,
  /(?:^|\/)gpt-5\.4-nano$/,
  /(?:^|\/)claude-fable-5$/,
  /(?:^|\/)claude-opus-4[-.]8$/,
  /(?:^|\/)claude-sonnet-4[-.]6$/,
  /(?:^|\/)claude-haiku-4[-.]5(?:-\d{8})?$/,
  /(?:^|\/)gemini-3\.1-pro-preview$/,
  /(?:^|\/)gemini-3\.5-flash$/,
  /(?:^|\/)gemini-3-flash-preview$/,
  /(?:^|\/)gemini-3\.1-flash-lite$/,
  /(?:^|\/)mistral-medium-3[-.]5/,
  /(?:^|\/)(?:mistral-small-4|mistral-small-latest)/,
  /(?:^|\/)(?:mistral-large-3|mistral-large-2512)/,
  /(?:^|\/)devstral-2/,
  /(?:^|\/)magistral-medium-(?:1[-.]2|2509)/,
  /(?:^|\/)ministral-(?:3|14b-2512|8b-2512|3b-2512)/,
] as const;

export const fetchJson = (url: string, headers: Record<string, string>) =>
  Effect.tryPromise({
    try: async () => {
      const r = await tauriFetch(url, { method: "GET", headers });
      if (!r.ok) {
        const errorBody = await r.text();
        throw new Error(`HTTP ${r.status}: ${errorBody}`);
      }
      return r.json();
    },
    catch: (e) => e,
  });

export const shouldIgnoreCommonKeywords = (id: string): boolean => {
  const lowerId = id.toLowerCase();
  return commonIgnoreKeywords.some((keyword) => lowerId.includes(keyword));
};

export const isDateSnapshot = (id: string): boolean => {
  if (/-\d{4}-\d{2}-\d{2}/.test(id)) return true;
  if (/-\d{8}$/.test(id)) return true;
  if (/-\d{4}$/.test(id)) return true;
  return false;
};

export const isNonChatModel = (id: string): boolean => {
  const lowerId = id.toLowerCase();
  const name = lowerId.includes("/") ? lowerId.split("/").pop()! : lowerId;

  if (/^o\d/.test(name)) return true;
  if (/^gpt-4o-/.test(name)) return true;
  if (/^gpt-4\.1/.test(name)) return true;
  if (name.startsWith("ft:") || lowerId.startsWith("ft:")) return true;
  if (/^gemma/.test(name)) return true;
  if (/^nano-banana/.test(name)) return true;

  return false;
};

export const isOldModel = (id: string): boolean => {
  const lowerId = id.toLowerCase();
  const name = lowerId.includes("/") ? lowerId.split("/").pop()! : lowerId;
  const dashedName = name.replace(/\./g, "-");

  if (/^gpt-3\.5/.test(name)) return true;
  if (/^gpt-4/.test(name)) return true;
  if (/^gpt-5($|-)/.test(name)) return true;
  if (/^gpt-5\.[1-3]($|-)/.test(name)) return true;
  if (/^(davinci|babbage|curie|ada)(-|$)/.test(name)) return true;
  if (/^claude-(2|3|instant)/.test(dashedName)) return true;
  if (
    /^claude-opus-4($|-)/.test(dashedName) &&
    !/^claude-opus-4-8($|-)/.test(dashedName)
  ) {
    return true;
  }
  if (/^claude-sonnet-4($|-)/.test(dashedName)) return true;
  if (
    /^claude-haiku-4($|-)/.test(dashedName) &&
    !/^claude-haiku-4-5($|-)/.test(dashedName)
  ) {
    return true;
  }
  if (/^gemini-(1|2)(\.|-)/.test(name)) return true;
  if (/^(open-)?mistral-(7b|nemo)(-|$)/.test(name)) return true;
  if (/^open-mixtral/.test(name)) return true;
  if (/^mistral-large-(24|240|241|2502|2508)/.test(name)) return true;
  if (/^mistral-medium-(2505|3[.-]1)($|-)/.test(name)) return true;
  if (/^mistral-small-(3|2503|2506)($|-)/.test(name)) return true;
  if (/^magistral-(small|medium)-2507($|-)/.test(name)) return true;
  if (/^ministral-(3b|8b)-24/.test(name)) return true;
  return false;
};

export const sortModelsByRecency = (models: string[]): string[] => {
  const priority = (model: string) => {
    const normalized = model.toLowerCase();
    const index = modelPriorityPatterns.findIndex((pattern) =>
      pattern.test(normalized),
    );
    return index === -1 ? modelPriorityPatterns.length : index;
  };

  return [...models].sort((a, b) => {
    const priorityDelta = priority(a) - priority(b);
    if (priorityDelta !== 0) {
      return priorityDelta;
    }
    return a.localeCompare(b);
  });
};

const hasMetadata = (metadata: ModelMetadata | undefined): boolean => {
  if (!metadata) {
    return false;
  }
  if (metadata.input_modalities && metadata.input_modalities.length > 0) {
    return true;
  }
  return false;
};

export const partition = <T>(
  items: readonly T[],
  shouldIgnore: (item: T) => ModelIgnoreReason[] | null,
  extract: (item: T) => string,
): { models: string[]; ignored: IgnoredModel[] } => {
  const models: string[] = [];
  const ignored: IgnoredModel[] = [];

  for (const item of items) {
    const reasons = shouldIgnore(item);
    const id = extract(item);

    if (!reasons || reasons.length === 0) {
      models.push(id);
    } else {
      ignored.push({ id, reasons });
    }
  }

  return { models, ignored };
};

export const extractMetadataMap = <T>(
  items: readonly T[],
  extract: (item: T) => string,
  extractMetadata: (item: T) => ModelMetadata | undefined,
): Record<string, ModelMetadata> => {
  const metadata: Record<string, ModelMetadata> = {};

  for (const item of items) {
    const id = extract(item);
    const meta = extractMetadata(item);
    if (hasMetadata(meta)) {
      metadata[id] = meta!;
    }
  }

  return metadata;
};
