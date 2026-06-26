import { tool } from "ai";
import { z } from "zod";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { SessionContentData } from "@hypr/plugin-fs-sync";

import { CONTEXT_TEXT_FIELD } from "./context-text";
import type { ToolDependencies } from "./types";

import type * as main from "~/store/tinybase/store/main";

const DEFAULT_READ_MAX_CHARS = 16_000;
const MAX_READ_CHARS = 30_000;
const DEFAULT_SEARCH_LIMIT = 5;
const MAX_SEARCH_LIMIT = 10;
const SNIPPET_RADIUS = 180;

type Store = ReturnType<typeof main.UI.useStore>;

type NoteSection = {
  title: string;
  text: string;
};

type LoadedNoteFile = {
  sessionId: string;
  title: string;
  date: string | null;
  eventName: string | null;
  eventId: string | null;
  participantIds: string[];
  participants: string[];
  sections: NoteSection[];
};

type SearchSnippet = {
  section: string;
  text: string;
};

type SearchMatch = {
  sessionId: string;
  title: string;
  date: string | null;
  score: number;
  snippets: SearchSnippet[];
};

const maxCharsSchema = z
  .number()
  .int()
  .min(1_000)
  .max(MAX_READ_CHARS)
  .optional()
  .describe("Maximum note content characters to return to the model");

function clampMaxChars(value: number | undefined): number {
  return Math.min(
    Math.max(value ?? DEFAULT_READ_MAX_CHARS, 1_000),
    MAX_READ_CHARS,
  );
}

function normalizeWhitespace(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

function extractEventName(event: unknown): string | null {
  if (!event || typeof event !== "object") {
    return null;
  }

  const record = event as Record<string, unknown>;
  if (typeof record.name === "string" && record.name.trim()) {
    return record.name.trim();
  }
  if (typeof record.title === "string" && record.title.trim()) {
    return record.title.trim();
  }

  return null;
}

function getParticipantName(store: Store, humanId: string): string | null {
  const row = store?.getRow("humans", humanId);
  const name = row?.name;
  return typeof name === "string" && name.trim() ? name.trim() : null;
}

function extractTranscriptText(
  transcript: SessionContentData["transcript"],
): string | null {
  const transcripts = transcript?.transcripts ?? [];
  const chunks = transcripts.flatMap((item) => {
    const memo = typeof item.memo_md === "string" ? item.memo_md.trim() : "";
    if (memo) {
      return [memo];
    }

    const words = item.words ?? [];
    const text = normalizeWhitespace(words.map((word) => word.text).join(" "));
    return text ? [text] : [];
  });

  return chunks.length > 0 ? chunks.join("\n\n") : null;
}

function buildNoteSections(payload: SessionContentData): NoteSection[] {
  const sections: NoteSection[] = [];

  if (payload.rawMemoMarkdown?.trim()) {
    sections.push({
      title: "Raw note",
      text: payload.rawMemoMarkdown.trim(),
    });
  }

  for (const note of payload.notes
    .slice()
    .sort((a, b) => (a.position ?? 0) - (b.position ?? 0))) {
    if (!note.markdown?.trim()) {
      continue;
    }

    sections.push({
      title: note.title?.trim() || "Enhanced note",
      text: note.markdown.trim(),
    });
  }

  const transcriptText = extractTranscriptText(payload.transcript);
  if (transcriptText) {
    sections.push({
      title: "Transcript",
      text: transcriptText,
    });
  }

  return sections;
}

function renderNoteContext(note: LoadedNoteFile): string {
  const header = [
    `# ${note.title || "Untitled"}`,
    note.date ? `Date: ${note.date}` : null,
    note.eventName ? `Event: ${note.eventName}` : null,
    note.participants.length > 0
      ? `Participants: ${note.participants.join(", ")}`
      : null,
  ].filter(Boolean);

  const body = note.sections.map(
    (section) => `## ${section.title}\n${section.text}`,
  );
  return [...header, ...body].join("\n\n");
}

function limitText(
  text: string,
  maxChars: number,
): {
  text: string;
  truncated: boolean;
} {
  if (text.length <= maxChars) {
    return { text, truncated: false };
  }

  return {
    text: `${text.slice(0, maxChars).trimEnd()}\n\n[Content truncated]`,
    truncated: true,
  };
}

async function loadNoteFile(
  sessionId: string,
  store: Store,
): Promise<LoadedNoteFile | null> {
  const result = await fsSyncCommands.loadSessionContent(sessionId);
  if (result.status === "error") {
    return null;
  }

  const payload = result.data;
  const participantIds =
    payload.meta?.participants
      ?.map((participant) => participant.humanId)
      .filter((humanId): humanId is string => Boolean(humanId)) ?? [];
  const participants = participantIds.flatMap((humanId) => {
    const name = getParticipantName(store, humanId);
    return name ? [name] : [];
  });

  return {
    sessionId,
    title: payload.meta?.title?.trim() || "Untitled",
    date: payload.meta?.createdAt ?? null,
    eventName: extractEventName(payload.meta?.event),
    eventId: payload.meta?.eventId ?? null,
    participantIds,
    participants,
    sections: buildNoteSections(payload),
  };
}

async function readNoteOutput({
  sessionId,
  store,
  maxChars,
}: {
  sessionId: string;
  store: Store;
  maxChars: number;
}) {
  const note = await loadNoteFile(sessionId, store);
  if (!note) {
    return {
      status: "error" as const,
      message: `Could not read note ${sessionId}`,
      sessionId,
    };
  }

  const fullText = renderNoteContext(note);
  const limited = limitText(fullText, maxChars);

  return {
    status: "ok" as const,
    sessionId: note.sessionId,
    title: note.title,
    date: note.date,
    event: note.eventName,
    participants: note.participants,
    sections: note.sections.map((section) => ({
      title: section.title,
      characters: section.text.length,
    })),
    truncated: limited.truncated,
    [CONTEXT_TEXT_FIELD]: limited.text,
  };
}

function getSessionIds(store: Store): string[] {
  const ids: string[] = [];
  store?.forEachRow("sessions", (rowId: string, _forEachCell: unknown) => {
    ids.push(rowId);
  });
  return ids;
}

function queryTerms(query: string): string[] {
  return Array.from(
    new Set(
      query
        .toLowerCase()
        .split(/[^a-z0-9@\-.]+/i)
        .map((term) => term.trim())
        .filter((term) => term.length >= 2),
    ),
  );
}

function createSnippet(text: string, index: number, length: number): string {
  const start = Math.max(0, index - SNIPPET_RADIUS);
  const end = Math.min(text.length, index + length + SNIPPET_RADIUS);
  const prefix = start > 0 ? "..." : "";
  const suffix = end < text.length ? "..." : "";
  return `${prefix}${normalizeWhitespace(text.slice(start, end))}${suffix}`;
}

function matchSection(
  section: NoteSection,
  query: string,
  terms: string[],
): {
  score: number;
  snippets: SearchSnippet[];
} {
  const lowerText = section.text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const snippets: SearchSnippet[] = [];
  let score = 0;

  const exactIndex = lowerText.indexOf(lowerQuery);
  if (lowerQuery && exactIndex >= 0) {
    score += 20;
    snippets.push({
      section: section.title,
      text: createSnippet(section.text, exactIndex, query.length),
    });
  }

  for (const term of terms) {
    const index = lowerText.indexOf(term);
    if (index < 0) {
      continue;
    }

    score += 3;
    if (snippets.length < 3) {
      snippets.push({
        section: section.title,
        text: createSnippet(section.text, index, term.length),
      });
    }
  }

  return { score, snippets };
}

function searchNote(note: LoadedNoteFile, query: string): SearchMatch | null {
  const terms = queryTerms(query);
  let score = 0;
  const snippets: SearchSnippet[] = [];
  const lowerQuery = query.toLowerCase();

  if (note.title.toLowerCase().includes(lowerQuery)) {
    score += 8;
    snippets.push({
      section: "Title",
      text: note.title,
    });
  }
  const matchingParticipants = note.participants.filter((name) =>
    name.toLowerCase().includes(lowerQuery),
  );
  if (matchingParticipants.length > 0) {
    score += 8;
    snippets.push({
      section: "Participants",
      text: matchingParticipants.join(", "),
    });
  }
  if (note.eventName?.toLowerCase().includes(lowerQuery)) {
    score += 8;
    snippets.push({
      section: "Event",
      text: note.eventName,
    });
  }

  for (const section of note.sections) {
    const match = matchSection(section, query, terms);
    score += match.score;
    snippets.push(...match.snippets);
  }

  if (score <= 0 || snippets.length === 0) {
    return null;
  }

  return {
    sessionId: note.sessionId,
    title: note.title,
    date: note.date,
    score,
    snippets: snippets.slice(0, 3),
  };
}

async function grepNoteFiles({
  query,
  sessionIds,
  limit,
  store,
}: {
  query: string;
  sessionIds?: string[];
  limit: number;
  store: Store;
}) {
  const trimmedQuery = query.trim();
  if (!trimmedQuery) {
    return {
      query,
      results: [],
      message: "Query is empty",
    };
  }

  const candidateIds = sessionIds?.length ? sessionIds : getSessionIds(store);
  const results: SearchMatch[] = [];

  for (const sessionId of candidateIds) {
    const note = await loadNoteFile(sessionId, store);
    if (!note) {
      continue;
    }

    const match = searchNote(note, trimmedQuery);
    if (match) {
      results.push(match);
    }
  }

  results.sort((a, b) => b.score - a.score);
  return {
    query: trimmedQuery,
    scanned: candidateIds.length,
    results: results.slice(0, limit),
  };
}

function sharedParticipantReasons(
  store: Store,
  baseIds: Set<string>,
  candidateIds: string[],
): string[] {
  return candidateIds.flatMap((humanId) => {
    if (!baseIds.has(humanId)) {
      return [];
    }

    const name = getParticipantName(store, humanId);
    return [`shared participant${name ? `: ${name}` : ""}`];
  });
}

function getDateDistanceDays(
  a: string | null,
  b: string | null,
): number | null {
  if (!a || !b) {
    return null;
  }

  const aMs = Date.parse(a);
  const bMs = Date.parse(b);
  if (!Number.isFinite(aMs) || !Number.isFinite(bMs)) {
    return null;
  }

  return Math.abs(aMs - bMs) / (24 * 60 * 60 * 1000);
}

async function listRelatedNotes({
  sessionId,
  store,
  limit,
}: {
  sessionId: string;
  store: Store;
  limit: number;
}) {
  const base = await loadNoteFile(sessionId, store);
  if (!base) {
    return {
      status: "error" as const,
      message: `Could not read note ${sessionId}`,
      sessionId,
      results: [],
    };
  }

  const baseParticipantIds = new Set(base.participantIds);
  const results: Array<{
    sessionId: string;
    title: string;
    date: string | null;
    score: number;
    reasons: string[];
  }> = [];

  for (const candidateId of getSessionIds(store)) {
    if (candidateId === sessionId) {
      continue;
    }

    const candidate = await loadNoteFile(candidateId, store);
    if (!candidate) {
      continue;
    }

    const reasons: string[] = [];
    let score = 0;

    if (base.eventId && candidate.eventId === base.eventId) {
      reasons.push("same calendar event");
      score += 20;
    }

    const participantReasons = sharedParticipantReasons(
      store,
      baseParticipantIds,
      candidate.participantIds,
    );
    if (participantReasons.length > 0) {
      reasons.push(...participantReasons);
      score += participantReasons.length * 8;
    }

    const distanceDays = getDateDistanceDays(base.date, candidate.date);
    if (distanceDays !== null && distanceDays <= 7) {
      reasons.push("nearby date");
      score += Math.max(1, 7 - Math.floor(distanceDays));
    }

    if (score > 0) {
      results.push({
        sessionId: candidate.sessionId,
        title: candidate.title,
        date: candidate.date,
        score,
        reasons,
      });
    }
  }

  results.sort((a, b) => b.score - a.score);
  return {
    status: "ok" as const,
    sessionId,
    title: base.title,
    results: results.slice(0, limit),
  };
}

export const buildReadCurrentNoteTool = (deps: ToolDependencies) =>
  tool({
    description:
      "Read the currently open note/meeting from local note files. Use this before answering questions about 'this note', 'this meeting', or the active note.",
    inputSchema: z.object({
      maxChars: maxCharsSchema,
    }),
    execute: async (params: { maxChars?: number }) => {
      const sessionId = deps.getSessionId();
      if (!sessionId) {
        return {
          status: "error" as const,
          message: "No note is currently open",
        };
      }

      return readNoteOutput({
        sessionId,
        store: deps.getStore(),
        maxChars: clampMaxChars(params.maxChars),
      });
    },
  });

export const buildReadNoteTool = (deps: ToolDependencies) =>
  tool({
    description:
      "Read a specific note/meeting by session id from local note files, including raw note, enhanced notes, transcript, participants, and event metadata.",
    inputSchema: z.object({
      sessionId: z.string().describe("Session id for the note to read"),
      maxChars: maxCharsSchema,
    }),
    execute: async (params: { sessionId: string; maxChars?: number }) =>
      readNoteOutput({
        sessionId: params.sessionId,
        store: deps.getStore(),
        maxChars: clampMaxChars(params.maxChars),
      }),
  });

export const buildGrepNotesTool = (deps: ToolDependencies) =>
  tool({
    description:
      "Lexically search local note files and transcripts for exact words or phrases. Use search_sessions first for open-ended questions about past meetings, people, decisions, or topics. This is filesystem-backed text search, not vector search.",
    inputSchema: z.object({
      query: z.string().describe("Text to search for in note files"),
      sessionIds: z
        .array(z.string())
        .optional()
        .describe("Optional session ids to restrict the file search"),
      limit: z
        .number()
        .int()
        .min(1)
        .max(MAX_SEARCH_LIMIT)
        .optional()
        .describe("Maximum matching notes to return"),
    }),
    execute: async (params: {
      query: string;
      sessionIds?: string[];
      limit?: number;
    }) =>
      grepNoteFiles({
        query: params.query,
        sessionIds: params.sessionIds,
        limit: Math.min(params.limit ?? DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT),
        store: deps.getStore(),
      }),
  });

export const buildListRelatedNotesTool = (deps: ToolDependencies) =>
  tool({
    description:
      "List notes related to the current or given note by shared participants, the same calendar event, or nearby dates. Use this when the user asks about people or related meetings.",
    inputSchema: z.object({
      sessionId: z
        .string()
        .optional()
        .describe(
          "Session id to find related notes for. Defaults to the currently open note.",
        ),
      limit: z
        .number()
        .int()
        .min(1)
        .max(MAX_SEARCH_LIMIT)
        .optional()
        .describe("Maximum related notes to return"),
    }),
    execute: async (params: { sessionId?: string; limit?: number }) => {
      const sessionId = params.sessionId ?? deps.getSessionId();
      if (!sessionId) {
        return {
          status: "error" as const,
          message: "No note is currently open",
          results: [],
        };
      }

      return listRelatedNotes({
        sessionId,
        store: deps.getStore(),
        limit: Math.min(params.limit ?? DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT),
      });
    },
  });

export const noteFileTestInternals = {
  buildNoteSections,
  queryTerms,
  searchNote,
};
