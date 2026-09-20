import {
  NoObjectGeneratedError,
  Output,
  streamText,
  type LanguageModel,
} from "ai";
import { z } from "zod";

import {
  commands as templateCommands,
  type JsonValue,
} from "@anlg/plugin-template";
import type {
  LiveTranscriptSegment,
  SegmentKey,
} from "@anlg/plugin-transcription";

import actionItemsSystemTemplate from "./live-assist-action-items.system.md.jinja?raw";
import actionItemsUserTemplate from "./live-assist-action-items.user.md.jinja?raw";
import catchUpSystemTemplate from "./live-assist-catch-up.system.md.jinja?raw";
import catchUpUserTemplate from "./live-assist-catch-up.user.md.jinja?raw";
import followUpSystemTemplate from "./live-assist-follow-up.system.md.jinja?raw";
import followUpUserTemplate from "./live-assist-follow-up.user.md.jinja?raw";
import summarizeSoFarSystemTemplate from "./live-assist-summarize-so-far.system.md.jinja?raw";
import summarizeSoFarUserTemplate from "./live-assist-summarize-so-far.user.md.jinja?raw";

export type LiveAssistKind =
  | "catch_up"
  | "action_items"
  | "follow_up"
  | "summarize_so_far";

export const LIVE_ASSIST_KINDS: readonly LiveAssistKind[] = [
  "catch_up",
  "action_items",
  "follow_up",
  "summarize_so_far",
];

// The live view keeps ~200 segments; that's plenty for a rolling 60s window,
// so the three "live" kinds can read `liveSegments` directly. Only
// summarize_so_far needs the persisted transcript (see hydrateSessionContext).
export const LIVE_ASSIST_CATCH_UP_WINDOW_MS = 60_000;
export const LIVE_ASSIST_WINDOW_MAX_CHARS = 4_000;
export const LIVE_ASSIST_SUMMARIZE_SO_FAR_MAX_CHARS = 6_000;
export const LIVE_ASSIST_MAX_ITEMS = 3;

const LIVE_ASSIST_GENERATION_TIMEOUT_MS = 18_000;
const LIVE_ASSIST_MAX_OUTPUT_TOKENS = 260;

const liveAssistSchema = z.object({
  items: z.array(z.string()).min(1).max(LIVE_ASSIST_MAX_ITEMS),
});

const LIVE_ASSIST_TEMPLATES: Record<
  LiveAssistKind,
  { system: string; user: string }
> = {
  catch_up: { system: catchUpSystemTemplate, user: catchUpUserTemplate },
  action_items: {
    system: actionItemsSystemTemplate,
    user: actionItemsUserTemplate,
  },
  follow_up: { system: followUpSystemTemplate, user: followUpUserTemplate },
  summarize_so_far: {
    system: summarizeSoFarSystemTemplate,
    user: summarizeSoFarUserTemplate,
  },
};

const LEADING_BULLET_REGEX = /^(?:[-*+]|\d+[.)])\s+/;

export function formatLiveAssistSpeakerLabel(key: SegmentKey): string {
  if (key.channel === "DirectMic") {
    return "You";
  }
  if (typeof key.speaker_index === "number") {
    return `Speaker ${key.speaker_index + 1}`;
  }
  return "Speaker";
}

export function buildLiveAssistWindow(
  segments: LiveTranscriptSegment[],
  sinceMs: number,
  nowMs: number,
  maxChars: number,
): string {
  const lines = segments
    .filter(
      (segment) =>
        segment.end_ms > sinceMs &&
        segment.start_ms < nowMs &&
        segment.text.trim(),
    )
    .slice()
    .sort((a, b) => a.start_ms - b.start_ms)
    .map(
      (segment) =>
        `${formatLiveAssistSpeakerLabel(segment.key)}: ${segment.text.trim()}`,
    );

  return truncateLiveAssistText(lines.join("\n"), maxChars);
}

export function buildSummarizeSoFarInput(
  transcript: { segments: Array<{ speaker: string; text: string }> } | null,
  maxChars: number,
): string {
  if (!transcript || transcript.segments.length === 0) {
    return "";
  }

  const lines = transcript.segments
    .map((segment) => `${segment.speaker}: ${segment.text}`.trim())
    .filter(Boolean);

  return truncateLiveAssistText(lines.join("\n"), maxChars);
}

// Keeps the most recent content (the end of the text) since that's what
// matters for a live suggestion; cuts on a line boundary when one is
// reasonably close to the start of the kept slice, in the same spirit as
// `compactBriefText`'s clean-cut truncation for the pre-meeting brief.
export function truncateLiveAssistText(text: string, maxChars: number): string {
  if (text.length <= maxChars) {
    return text;
  }

  const slice = text.slice(text.length - maxChars);
  const firstNewline = slice.indexOf("\n");
  if (firstNewline === -1 || firstNewline > maxChars * 0.4) {
    return slice;
  }
  return slice.slice(firstNewline + 1);
}

export function sanitizeLiveAssistItem(text: string | undefined): string {
  return (
    text?.replace(LEADING_BULLET_REGEX, "").replace(/\*+/g, "").trim() ?? ""
  );
}

export function sanitizeLiveAssistItems(
  items: Array<string | undefined>,
): string[] {
  const seen = new Set<string>();
  const result: string[] = [];

  for (const raw of items) {
    const item = sanitizeLiveAssistItem(raw);
    if (!item) {
      continue;
    }
    const key = item.toLowerCase();
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(item);
    if (result.length >= LIVE_ASSIST_MAX_ITEMS) {
      break;
    }
  }

  return result;
}

function extractLiveAssistItemsFromText(text: string): string[] {
  const lines = text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const bulletLines = lines.filter((line) => LEADING_BULLET_REGEX.test(line));
  return sanitizeLiveAssistItems(bulletLines.length > 0 ? bulletLines : lines);
}

// Only `replaceContent` exists on the note editor commands (see
// `packages/editor/src/note/index.tsx`); there is no "insert at end" API.
// Appending to the persisted markdown and letting the editor re-render from
// it is the pragmatic v1 substitute, in the same spirit as
// `mergeBriefMarkdown` (which prepends instead, for the pre-meeting brief).
export function appendLiveAssistMarkdown(
  existing: string,
  addition: string,
): string {
  const nextExisting = existing.trim();
  const nextAddition = addition.trim();
  if (!nextAddition) {
    return nextExisting;
  }
  if (!nextExisting) {
    return nextAddition;
  }
  return `${nextExisting}\n\n${nextAddition}`;
}

export function formatLiveAssistCardMarkdown(
  kindLabel: string,
  items: string[],
): string {
  const bullets = items.map((item) => `- ${item}`).join("\n");
  return [`**${kindLabel}**`, bullets].filter(Boolean).join("\n\n");
}

export async function streamLiveAssistSuggestion({
  model,
  language,
  kind,
  sourceText,
  signal,
}: {
  model: LanguageModel;
  language: string;
  kind: LiveAssistKind;
  sourceText: string;
  signal?: AbortSignal;
}): Promise<string[]> {
  if (!sourceText.trim()) {
    return [];
  }

  const templates = LIVE_ASSIST_TEMPLATES[kind];
  const system = await renderJinja(templates.system, { language });
  const prompt = await renderJinja(templates.user, {
    transcript: sourceText,
  });

  const result = streamText({
    model,
    system,
    prompt,
    output: Output.object({ schema: liveAssistSchema }),
    abortSignal: signal,
    maxRetries: 1,
    maxOutputTokens: LIVE_ASSIST_MAX_OUTPUT_TOKENS,
    timeout: { totalMs: LIVE_ASSIST_GENERATION_TIMEOUT_MS },
  });

  try {
    const output = await result.output;
    return sanitizeLiveAssistItems(output?.items ?? []);
  } catch (error) {
    if (!NoObjectGeneratedError.isInstance(error)) {
      throw error;
    }

    const fallbackItems = extractLiveAssistItemsFromText(error.text ?? "");
    if (fallbackItems.length === 0) {
      throw error;
    }
    return fallbackItems;
  }
}

type TemplateContext = Partial<{ [key: string]: JsonValue }>;

async function renderJinja(templateContent: string, ctx: TemplateContext) {
  const result = await templateCommands.renderCustom(templateContent, ctx);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}
