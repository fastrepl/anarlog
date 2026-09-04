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
import { parseEventInstant } from "@anlg/utils";

import systemPromptTemplate from "./pre-meeting-brief.system.md.jinja?raw";
import userPromptTemplate from "./pre-meeting-brief.user.md.jinja?raw";

import { extractPlainText } from "~/search/contexts/engine/utils";
import type { PastSessionNote } from "~/session/insights/past-notes";

export const MAX_BRIEF_MEETINGS = 5;

const AFTER_START_GRACE_MS = 5 * 60 * 1000;
const MAX_FACTS = 3;
const MAX_PROMPT_FACTS = 4;
const MAX_BRIEF_BULLETS = 3;
const MAX_SUMMARY_LENGTH = 320;
const BRIEF_GENERATION_TIMEOUT_MS = 45_000;
const BRIEF_MAX_OUTPUT_TOKENS = 512;
const briefSchema = z.object({
  opener: z.string(),
  bullets: z.array(z.string()).min(1).max(MAX_BRIEF_BULLETS),
});
const SECTION_LABEL_REGEX =
  /^(quick\s+)?(recap|summary|overview|agenda|insights?|upcoming|next steps|prepare|brief)\b/i;
const INSTRUCTION_LEFTOVER_REGEX =
  /one sentence|why this conversation matters|would not immediately remember|open loop or commitment|thing to listen for or decide|this (meeting|sync|conversation) is (crucial|important|needed|essential)|aligning the team's vision/i;
const SPACE_REGEX = /\s+/g;

export type PreMeetingBriefEvent = {
  title?: string;
  started_at?: string;
  ended_at?: string;
  is_all_day?: boolean;
  location?: string;
  description?: string;
  participants?: Array<{
    name?: string;
    email?: string;
    is_current_user?: boolean;
  }>;
};

export function shouldShowPreMeetingBrief(
  event: {
    started_at?: string;
    ended_at?: string;
    is_all_day?: boolean;
  } | null,
  nowMs: number,
): boolean {
  if (!event || event.is_all_day) {
    return false;
  }

  const startMs = parseEventInstant(event.started_at)?.getTime();
  if (startMs == null) {
    return false;
  }
  if (startMs > nowMs) {
    return true;
  }

  const endMs = parseEventInstant(event.ended_at)?.getTime();
  const hideAfterMs =
    endMs == null
      ? startMs + AFTER_START_GRACE_MS
      : Math.max(endMs, startMs + AFTER_START_GRACE_MS);
  return hideAfterMs > nowMs;
}

export function selectBriefSourceNotes(
  notes: PastSessionNote[],
): PastSessionNote[] {
  return notes
    .filter((note) => getPreMeetingBriefFacts(note).length > 0)
    .slice(0, MAX_BRIEF_MEETINGS);
}

export function canCreatePreMeetingBrief({
  event,
  nowMs,
  notes,
  hasParticipants = false,
}: {
  event: PreMeetingBriefEvent | null;
  nowMs: number;
  notes: PastSessionNote[];
  hasParticipants?: boolean;
}): boolean {
  return (
    (shouldShowPreMeetingBrief(event, nowMs) || hasParticipants) &&
    selectBriefSourceNotes(notes).length > 0
  );
}

export function getBriefEventParticipantNames(
  event: PreMeetingBriefEvent | null,
): string[] {
  return [
    ...new Set(
      (event?.participants ?? [])
        .filter((participant) => participant.is_current_user !== true)
        .map(
          (participant) =>
            participant.name?.trim() || participant.email?.trim() || "",
        )
        .filter(Boolean),
    ),
  ].slice(0, 8);
}

export function getPreMeetingBriefFacts(note: PastSessionNote): string[] {
  const generatedFacts = note.summary
    ? note.summary
        .split("\n")
        .map((line) => compactBriefText(line, MAX_SUMMARY_LENGTH))
        .filter(Boolean)
        .slice(0, MAX_FACTS)
    : [];
  if (generatedFacts.length > 0) {
    return [...new Set(generatedFacts)];
  }

  const sourceSummary = compactBriefText(
    note.sourceSummary,
    MAX_SUMMARY_LENGTH,
  );
  return sourceSummary ? [sourceSummary] : [];
}

export function compactBriefText(value: string, maxLength: number): string {
  const text = extractPlainText(value)
    .replace(/<[^>]*>/g, " ")
    .replace(/!\[[^\]]*]\([^)]+\)/g, " ")
    .replace(/\[([^\]]+)]\([^)]+\)/g, "$1")
    .replace(/https?:\/\/\S+/gi, " ")
    .replace(/(^|\s)([-*+]|\d+[.)])\s+/g, " ")
    .replace(/[`_~>#]/g, "")
    .replace(SPACE_REGEX, " ")
    .trim();
  if (text.length <= maxLength) {
    return text;
  }

  const slice = text.slice(0, maxLength + 1);
  const lastSpace = slice.lastIndexOf(" ");
  const end = lastSpace > maxLength * 0.6 ? lastSpace : maxLength;
  return `${slice.slice(0, end).trim()}…`;
}

export function formatPreMeetingBrief(brief: {
  opener?: string;
  bullets?: Array<string | undefined>;
}): string {
  const opener = sanitizeBriefOpener(brief.opener);
  const bullets = (brief.bullets ?? [])
    .map(sanitizeBriefBullet)
    .filter(Boolean)
    .slice(0, MAX_BRIEF_BULLETS)
    .map((item) => `- ${item}`);

  return [opener && `**${opener}**`, bullets.join("\n")]
    .filter(Boolean)
    .join("\n\n");
}

export function trimPreMeetingBrief(text: string): string {
  const openerParts: string[] = [];
  const bullets: string[] = [];

  for (const rawLine of text.split("\n")) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }

    const heading = line.match(/^#{1,6}\s+(.+)$/);
    const bullet = line.match(/^(?:[-*+]|\d+[.)])\s+(.+)$/);
    if (bullet) {
      const item = sanitizeBriefBullet(bullet[1]);
      if (item && bullets.length < MAX_BRIEF_BULLETS) {
        bullets.push(`- ${item}`);
      }
      continue;
    }

    if (bullets.length > 0 || openerParts.length > 0) {
      continue;
    }

    const body = sanitizeBriefOpener(heading?.[1] ?? line);
    if (body) {
      openerParts.push(`**${body}**`);
    }
  }

  return [openerParts[0], bullets.join("\n")].filter(Boolean).join("\n\n");
}

function sanitizeBriefOpener(text: string | undefined): string {
  const body = text?.replace(/\*+/g, "").trim() ?? "";
  if (!body || isBriefSectionLabel(body) || isBriefInstructionLeftover(body)) {
    return "";
  }
  return body;
}

function sanitizeBriefBullet(text: string | undefined): string {
  const item =
    text
      ?.replace(/^(?:[-*+]|\d+[.)])\s+/, "")
      .replace(/\*+/g, "")
      .trim() ?? "";
  if (!item || isBriefInstructionLeftover(item)) {
    return "";
  }
  return item;
}

function isBriefSectionLabel(text: string): boolean {
  const plain = text.replace(/[#*_]/g, "").trim();
  if (!plain) {
    return true;
  }
  if (plain.length < 48 && /:$/.test(plain)) {
    return true;
  }
  return SECTION_LABEL_REGEX.test(plain);
}

function isBriefInstructionLeftover(text: string): boolean {
  return INSTRUCTION_LEFTOVER_REGEX.test(text.replace(/[#*_]/g, "").trim());
}

function getBriefPromptMeetings(notes: PastSessionNote[]) {
  let remaining = MAX_PROMPT_FACTS;
  return notes.flatMap((note) => {
    if (remaining <= 0) {
      return [];
    }
    const facts = getPreMeetingBriefFacts(note).slice(0, remaining);
    remaining -= facts.length;
    if (facts.length === 0) {
      return [];
    }
    return [
      {
        title: note.title,
        date: note.dateLabel,
        participants: note.participantNames ?? [],
        notes: facts.join("\n"),
      },
    ];
  });
}

export function mergeBriefMarkdown(brief: string, existing: string): string {
  const nextBrief = brief.trim();
  const nextExisting = existing.trim();
  if (!nextBrief) {
    return nextExisting;
  }
  if (!nextExisting) {
    return nextBrief;
  }
  return `${nextBrief}\n\n${nextExisting}`;
}

export async function streamPreMeetingBrief({
  model,
  language,
  event,
  notes,
  onText,
  signal,
}: {
  model: LanguageModel;
  language: string;
  event: PreMeetingBriefEvent;
  notes: PastSessionNote[];
  onText?: (text: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const sourceNotes = selectBriefSourceNotes(notes);
  if (sourceNotes.length === 0) {
    return "";
  }

  const system = await renderJinja(systemPromptTemplate, { language });
  const prompt = await renderJinja(userPromptTemplate, {
    meeting: {
      title: event.title?.trim() || "Untitled",
      when: compactBriefText(
        [event.started_at, event.ended_at].filter(Boolean).join(" – "),
        160,
      ),
      location: compactBriefText(event.location ?? "", 120),
      participants: getBriefEventParticipantNames(event),
      description: compactBriefText(event.description ?? "", 400),
    },
    past_meetings: getBriefPromptMeetings(sourceNotes),
  });

  const result = streamText({
    model,
    system,
    prompt,
    output: Output.object({ schema: briefSchema }),
    abortSignal: signal,
    maxRetries: 2,
    maxOutputTokens: BRIEF_MAX_OUTPUT_TOKENS,
    timeout: { totalMs: BRIEF_GENERATION_TIMEOUT_MS },
  });

  for await (const partial of result.partialOutputStream) {
    const markdown = formatPreMeetingBrief(partial);
    if (markdown) {
      onText?.(markdown);
    }
  }

  try {
    return formatPreMeetingBrief((await result.output) ?? {});
  } catch (error) {
    if (
      !NoObjectGeneratedError.isInstance(error) ||
      error.finishReason === "length"
    ) {
      throw error;
    }

    const brief = trimPreMeetingBrief(error.text ?? "");
    const bulletCount = brief.match(/^- /gm)?.length ?? 0;
    if (bulletCount !== MAX_BRIEF_BULLETS) {
      throw error;
    }
    return brief;
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
