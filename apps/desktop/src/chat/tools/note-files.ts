import { tool } from "ai";
import { z } from "zod";

import type { ToolDependencies } from "./types";

import {
  loadActiveSessionIds,
  loadSessionContentSnapshot,
  type SessionContentSnapshot,
} from "~/session/content-queries";
import { loadSessionSummariesByFolder } from "~/session/queries";
import {
  formatMeetingChatRecordsAsMarkdown,
  loadMeetingChatRecords,
} from "~/stt/meeting-chat-records";

const DEFAULT_SEARCH_LIMIT = 5;
const MAX_SEARCH_LIMIT = 10;
const SNIPPET_RADIUS = 180;

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
  participantNamesById?: Record<string, string>;
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

function extractTranscriptText(
  transcripts: SessionContentSnapshot["transcripts"],
): string | null {
  const chunks = transcripts.flatMap((item) => {
    const memo = item.memo.trim();
    if (memo) {
      return [memo];
    }

    const words = item.words ?? [];
    const text = normalizeWhitespace(words.map((word) => word.text).join(" "));
    return text ? [text] : [];
  });

  return chunks.length > 0 ? chunks.join("\n\n") : null;
}

function buildNoteSections(
  snapshot: SessionContentSnapshot,
  meetingChatMarkdown = "",
): NoteSection[] {
  const sections: NoteSection[] = [];

  if (snapshot.rawMarkdown.trim()) {
    sections.push({
      title: "Raw note",
      text: snapshot.rawMarkdown.trim(),
    });
  }

  if (meetingChatMarkdown.trim()) {
    sections.push({
      title: "Meeting chat",
      text: meetingChatMarkdown.trim(),
    });
  }

  for (const note of snapshot.enhancedNotes) {
    if (!note.markdown.trim()) {
      continue;
    }

    sections.push({
      title: note.title.trim() || "Enhanced note",
      text: note.markdown.trim(),
    });
  }

  const transcriptText = extractTranscriptText(snapshot.transcripts);
  if (transcriptText) {
    sections.push({
      title: "Transcript",
      text: transcriptText,
    });
  }

  return sections;
}

async function loadNoteFile(sessionId: string): Promise<LoadedNoteFile | null> {
  const snapshot = await loadSessionContentSnapshot(sessionId);
  if (!snapshot) return null;
  const meetingChatMarkdown = formatMeetingChatRecordsAsMarkdown(
    await loadMeetingChatRecords(sessionId),
  );

  const participantIds = snapshot.participants.map(
    (participant) => participant.humanId,
  );
  const participants = snapshot.participants
    .map((participant) => participant.name.trim())
    .filter(Boolean);
  const participantNamesById = Object.fromEntries(
    snapshot.participants.flatMap((participant) =>
      participant.name.trim()
        ? [[participant.humanId, participant.name.trim()]]
        : [],
    ),
  );

  return {
    sessionId,
    title: snapshot.title.trim() || "Untitled",
    date: snapshot.createdAt || null,
    eventName: extractEventName(snapshot.event),
    eventId: snapshot.eventId,
    participantIds,
    participants,
    participantNamesById,
    sections: buildNoteSections(snapshot, meetingChatMarkdown),
  };
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

export async function searchMeetingContent({
  query,
  sessionIds,
  limit,
}: {
  query: string;
  sessionIds?: string[];
  limit: number;
}) {
  const trimmedQuery = query.trim();
  if (!trimmedQuery) {
    return {
      query,
      results: [],
      message: "Query is empty",
    };
  }

  const candidateIds =
    sessionIds === undefined ? await loadActiveSessionIds() : sessionIds;
  const results: SearchMatch[] = [];

  for (const sessionId of candidateIds) {
    const note = await loadNoteFile(sessionId);
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
  baseIds: Set<string>,
  candidate: LoadedNoteFile,
): string[] {
  return candidate.participantIds.flatMap((humanId) => {
    if (!baseIds.has(humanId)) {
      return [];
    }

    const name = candidate.participantNamesById?.[humanId];
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
  limit,
}: {
  sessionId: string;
  limit: number;
}) {
  const base = await loadNoteFile(sessionId);
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

  for (const candidateId of await loadActiveSessionIds()) {
    if (candidateId === sessionId) {
      continue;
    }

    const candidate = await loadNoteFile(candidateId);
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
      baseParticipantIds,
      candidate,
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

export const buildSearchMeetingContentTool = (deps: ToolDependencies) =>
  tool({
    description:
      "Search local meeting notes and transcripts for exact words or phrases. Use search_meetings first for open-ended questions about past meetings, people, decisions, or topics. This is lexical content search, not vector search.",
    inputSchema: z.object({
      query: z.string().describe("Text to search for in meeting content"),
      meeting_ids: z
        .array(z.string())
        .optional()
        .describe("Optional meeting ids to restrict the content search"),
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
      meeting_ids?: string[];
      limit?: number;
    }) => {
      const folderFilter = deps.getFolderFilter?.() ?? null;
      const folderSessionIds =
        folderFilter === null
          ? undefined
          : (await loadSessionSummariesByFolder(folderFilter)).map(
              (session) => session.id,
            );
      const sessionIds =
        folderSessionIds === undefined
          ? params.meeting_ids
          : params.meeting_ids?.length
            ? params.meeting_ids.filter((id) => folderSessionIds.includes(id))
            : folderSessionIds;
      const result = await searchMeetingContent({
        query: params.query,
        sessionIds,
        limit: Math.min(params.limit ?? DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT),
      });
      return {
        ...result,
        results: result.results.map(({ sessionId, ...match }) => ({
          ...match,
          meeting_id: sessionId,
        })),
      };
    },
  });

export const buildFindRelatedMeetingsTool = (deps: ToolDependencies) =>
  tool({
    description:
      "Find meetings related to the current or given meeting by shared participants, the same calendar event, or nearby dates.",
    inputSchema: z.object({
      meeting_id: z
        .string()
        .optional()
        .describe(
          "Meeting id to find related meetings for. Defaults to the currently open meeting.",
        ),
      limit: z
        .number()
        .int()
        .min(1)
        .max(MAX_SEARCH_LIMIT)
        .optional()
        .describe("Maximum related meetings to return"),
    }),
    execute: async (params: { meeting_id?: string; limit?: number }) => {
      const sessionId = params.meeting_id ?? deps.getSessionId();
      if (!sessionId) {
        return {
          status: "error" as const,
          message: "No meeting is currently open",
          results: [],
        };
      }

      const result = await listRelatedNotes({
        sessionId,
        limit: Math.min(params.limit ?? DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT),
      });
      const { sessionId: resolvedMeetingId, ...related } = result;
      return {
        ...related,
        meeting_id: resolvedMeetingId,
        results: result.results.map(({ sessionId, ...meeting }) => ({
          ...meeting,
          meeting_id: sessionId,
        })),
      };
    },
  });

export const noteFileTestInternals = {
  buildNoteSections,
  queryTerms,
  searchNote,
};
