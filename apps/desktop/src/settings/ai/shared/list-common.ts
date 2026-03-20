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

export const commonIgnoreKeywords = [
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
  if (/^gpt-3\.5/.test(lowerId)) return true;
  if (/^gpt-4(?!o|\.)/.test(lowerId)) return true;
  if (/^(davinci|babbage|curie|ada)(-|$)/.test(lowerId)) return true;
  if (/^claude-(2|instant)/.test(lowerId)) return true;
  return false;
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
