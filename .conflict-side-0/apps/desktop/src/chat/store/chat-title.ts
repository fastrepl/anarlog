import { generateText, type LanguageModel } from "ai";

import { deterministicGenerationSettings } from "~/ai/model-settings";

const FALLBACK_CHAT_TITLE_MAX_LENGTH = 50;
const GENERATED_CHAT_TITLE_MAX_LENGTH = 60;
const INITIAL_REQUEST_MAX_LENGTH = 4000;

export function createFallbackChatTitle(initialRequest: string): string {
  const title = normalizeTitleText(initialRequest);

  if (!title) {
    return "New chat";
  }

  return truncateTitle(title, FALLBACK_CHAT_TITLE_MAX_LENGTH);
}

export async function generateChatTitle({
  model,
  initialRequest,
}: {
  model: LanguageModel;
  initialRequest: string;
}): Promise<string | null> {
  const request = normalizeTitleText(initialRequest).slice(
    0,
    INITIAL_REQUEST_MAX_LENGTH,
  );

  if (!request) {
    return null;
  }

  const result = await generateText({
    model,
    ...deterministicGenerationSettings(model),
    maxRetries: 2,
    maxOutputTokens: 32,
    system:
      "Write a concise chat title from the user's first message. Use the same language as the request. Return only the title, with no quotes, emoji, markdown, or ending punctuation. Keep it under 6 words.",
    prompt: `Initial request:\n${request}`,
  });

  return normalizeGeneratedChatTitle(result.text);
}

export function normalizeGeneratedChatTitle(text: string): string | null {
  const firstLine = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);

  if (!firstLine) {
    return null;
  }

  const title = normalizeTitleText(firstLine)
    .replace(/^\d+[.)]\s*/, "")
    .replace(/^[-*#]\s*/, "")
    .replace(/^["'`]+/, "")
    .replace(/["'`]+$/, "")
    .replace(/[.!?]+$/, "")
    .trim();

  if (!title) {
    return null;
  }

  return truncateTitle(title, GENERATED_CHAT_TITLE_MAX_LENGTH);
}

function normalizeTitleText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

function truncateTitle(title: string, maxLength: number): string {
  if (title.length <= maxLength) {
    return title;
  }

  const truncated = title.slice(0, maxLength - 3).trimEnd();
  const lastSpace = truncated.lastIndexOf(" ");
  const titlePrefix =
    lastSpace > Math.floor(maxLength / 2)
      ? truncated.slice(0, lastSpace)
      : truncated;

  return `${titlePrefix}...`;
}
