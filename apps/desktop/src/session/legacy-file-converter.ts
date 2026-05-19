import { isValidContent, json2md } from "@hypr/editor/markdown";

import {
  SESSION_MARKDOWN_SCHEMA_VERSION,
  type SessionMarkdownDocument,
  type SessionMarkdownParticipant,
} from "./file-format";

import type { LoadedSessionData } from "~/store/tinybase/persister/session/load";

const FALLBACK_CREATED_AT = "1970-01-01T00:00:00.000Z";

type LegacyEvent = {
  event?: Record<string, unknown>;
  eventId?: string;
};

type LegacyTranscriptWord = {
  text?: unknown;
  speaker?: unknown;
};

export function legacySessionToMarkdownDocument(
  data: LoadedSessionData,
  sessionId: string,
): SessionMarkdownDocument | null {
  const session = data.sessions[sessionId];
  if (!session) return null;

  const legacyEvent = parseLegacyEvent(session.event_json);

  return {
    schemaVersion: SESSION_MARKDOWN_SCHEMA_VERSION,
    id: sessionId,
    createdAt: session.created_at || FALLBACK_CREATED_AT,
    title: session.title ?? "",
    folderId: session.folder_id || undefined,
    eventId: legacyEvent.eventId,
    event: legacyEvent.event,
    participants: collectParticipants(data, sessionId),
    tags: collectTags(data, sessionId),
    notes: legacyRichTextToMarkdown(session.raw_md),
    summary: collectSummary(data, sessionId),
    transcript: collectTranscript(data, sessionId),
  };
}

function legacyRichTextToMarkdown(content: unknown): string {
  if (typeof content !== "string") return "";

  const trimmed = content.trim();
  if (!trimmed) return "";

  try {
    const parsed = JSON.parse(trimmed);
    if (!isValidContent(parsed)) return trimmed;
    return json2md(parsed).trim();
  } catch {
    return trimmed;
  }
}

function collectParticipants(
  data: LoadedSessionData,
  sessionId: string,
): SessionMarkdownParticipant[] {
  return Object.values(data.mapping_session_participant)
    .filter((participant) => participant.session_id === sessionId)
    .map((participant) => ({
      legacy_human_id: participant.human_id || undefined,
      source: participant.source || undefined,
    }));
}

function collectTags(data: LoadedSessionData, sessionId: string): string[] {
  return Object.values(data.mapping_tag_session)
    .filter((mapping) => mapping.session_id === sessionId)
    .map((mapping) => {
      if (!mapping.tag_id) return undefined;
      return data.tags[mapping.tag_id]?.name || mapping.tag_id;
    })
    .filter((tag): tag is string => Boolean(tag));
}

function collectSummary(data: LoadedSessionData, sessionId: string): string {
  const notes = Object.entries(data.enhanced_notes)
    .filter(([, note]) => note.session_id === sessionId)
    .sort(([leftId, left], [rightId, right]) => {
      const byPosition = (left.position ?? 0) - (right.position ?? 0);
      return byPosition || leftId.localeCompare(rightId);
    })
    .flatMap(([, note]) => {
      const content = legacyRichTextToMarkdown(note.content);
      if (!content) return [];

      return [
        {
          title: note.title?.trim(),
          content,
        },
      ];
    });

  if (notes.length === 1 && !notes[0].title) {
    return notes[0].content;
  }

  return notes
    .map((note) =>
      note.title ? `## ${note.title}\n\n${note.content}` : note.content,
    )
    .join("\n\n");
}

function collectTranscript(data: LoadedSessionData, sessionId: string): string {
  return Object.entries(data.transcripts)
    .filter(([, transcript]) => transcript.session_id === sessionId)
    .sort(([leftId, left], [rightId, right]) => {
      const byStart = (left.started_at ?? 0) - (right.started_at ?? 0);
      return byStart || leftId.localeCompare(rightId);
    })
    .map(([, transcript]) => {
      const spoken = renderWords(parseTranscriptWords(transcript.words));
      const memo = transcript.memo_md?.trim();

      if (spoken && memo) return `${spoken}\n\n${memo}`;
      return spoken || memo || "";
    })
    .filter(Boolean)
    .join("\n\n");
}

function parseTranscriptWords(content: unknown): LegacyTranscriptWord[] {
  if (typeof content !== "string" || !content.trim()) return [];

  try {
    const parsed = JSON.parse(content);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function renderWords(words: LegacyTranscriptWord[]): string {
  const lines: string[] = [];
  let currentSpeaker = "";
  let currentWords: string[] = [];

  function flush(): void {
    if (currentWords.length === 0) return;

    const text = currentWords.join(" ").replace(/\s+([.,!?;:])/g, "$1");
    lines.push(currentSpeaker ? `${currentSpeaker}: ${text}` : text);
    currentWords = [];
  }

  for (const word of words) {
    const text = typeof word.text === "string" ? word.text.trim() : "";
    if (!text) continue;

    const speaker = typeof word.speaker === "string" ? word.speaker.trim() : "";
    if (speaker !== currentSpeaker) {
      flush();
      currentSpeaker = speaker;
    }
    currentWords.push(text);
  }
  flush();

  return lines.join("\n");
}

function parseLegacyEvent(content: unknown): LegacyEvent {
  const raw = typeof content === "string" ? content.trim() : "";
  if (!raw) return {};

  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed === "string") {
      return { eventId: parsed || undefined };
    }
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }

    const event = parsed as Record<string, unknown>;
    return {
      event,
      eventId: extractEventId(event),
    };
  } catch {
    return { eventId: raw };
  }
}

function extractEventId(event: Record<string, unknown>): string | undefined {
  for (const key of ["tracking_id_event", "tracking_id", "id"]) {
    const value = event[key];
    if (typeof value === "string" && value) return value;
  }
}
