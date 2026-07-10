import { useIsMutating, useMutation } from "@tanstack/react-query";
import { generateText, type LanguageModel, Output } from "ai";
import { useCallback, useMemo } from "react";
import { z } from "zod";

import {
  commands as templateCommands,
  type JsonValue,
} from "@hypr/plugin-template";
import { format, safeParseDate } from "@hypr/utils";

import systemPromptTemplate from "./past-note-key-facts.system.md.jinja?raw";
import userPromptTemplate from "./past-note-key-facts.user.md.jinja?raw";

import { useLanguageModel } from "~/ai/hooks";
import { deterministicGenerationSettings } from "~/ai/model-settings";
import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { extractPlainText } from "~/search/contexts/engine/utils";
import { getSessionEvent } from "~/session/utils";
import { showTransientToast } from "~/sidebar/toast/transient";

export type PastSessionNote = {
  sessionId: string;
  title: string;
  dateLabel: string;
  participantNames?: string[];
  summary: string | null;
  isGenerating: boolean;
  isRegenerateDisabled?: boolean;
};

export type PastSessionNoteRequest = {
  sessionId: string;
  userId: string;
  title: string;
  dateLabel: string;
  sourceText: string;
  sourceHash: string;
  participantNames: string[];
};

export type PastSessionNotesResult = {
  notes: PastSessionNote[];
  hasPastNotes: boolean;
  isGenerating: boolean;
  canGenerate: boolean;
  regenerate: (sessionId: string) => void;
  regenerateAll: () => void;
};

type PastSessionRow = {
  id: string;
  user_id: string;
  title: string;
  created_at: string;
  event_json: string;
};

type PastParticipantRow = {
  session_id: string;
  human_id: string;
  user_id: string;
  source: string;
  name: string;
};

type PastEnhancedNoteRow = {
  session_id: string;
  content: string;
  position: number;
};

type PastKeyFactsRow = {
  session_id: string;
  user_id: string;
  created_at: string;
  updated_at: string;
  content: string;
  source_hash: string;
};

export type PastSessionNotesData = {
  sessions: Record<string, PastSessionRow>;
  participants: PastParticipantRow[];
  enhancedNotes: PastEnhancedNoteRow[];
  keyFacts: Record<string, PastKeyFactsRow>;
};

const MAX_PAST_NOTES = 8;
const MAX_SOURCE_LENGTH = 6000;
const MAX_KEY_FACTS = 3;
const KEY_FACTS_GENERATION_TIMEOUT_MS = 30_000;
const SPACE_REGEX = /\s+/g;
const GENERIC_TITLE_KEYS = new Set(["new note", "untitled"]);
const EMPTY_SESSIONS: Record<string, PastSessionRow> = {};
const EMPTY_PARTICIPANTS: PastParticipantRow[] = [];
const EMPTY_ENHANCED_NOTES: PastEnhancedNoteRow[] = [];
const EMPTY_KEY_FACTS: Record<string, PastKeyFactsRow> = {};

const keyFactsSchema = z.object({
  facts: z.array(z.string()).min(1).max(MAX_KEY_FACTS),
});

export function usePastSessionNotes(
  sessionId: string,
  { enabled = true }: { enabled?: boolean } = {},
): PastSessionNotesResult {
  const mutationKey = useMemo(
    () => ["past-session-notes", sessionId],
    [sessionId],
  );
  const activeMutationCount = useIsMutating({ mutationKey });
  const { sessions, participants, enhancedNotes, keyFacts } =
    usePastSessionNotesData(enabled);
  const userId = sessions[sessionId]?.user_id || null;
  const model = useLanguageModel("enhance");

  const built = useMemo(() => {
    if (!enabled) {
      return { notes: [], missing: [], requests: [] };
    }

    return buildPastSessionNotes(
      { sessions, participants, enhancedNotes, keyFacts },
      sessionId,
      userId,
    );
  }, [
    enabled,
    sessionId,
    userId,
    sessions,
    participants,
    enhancedNotes,
    keyFacts,
  ]);

  const mutation = useMutation({
    mutationKey,
    mutationFn: async (requests: PastSessionNoteRequest[]) => {
      if (!model || requests.length === 0) {
        return;
      }

      await generateAndSavePastSessionNotes({
        model,
        requests,
      });
    },
    onError: (error) => {
      console.error("Failed to generate meeting insights", error);
      showTransientToast({
        id: "past-note-key-facts-error",
        description: "Could not generate meeting insights. Try again.",
        variant: "error",
      });
    },
  });

  const generatingIds = useMemo(
    () =>
      mutation.isPending && mutation.variables
        ? new Set(mutation.variables.map((request) => request.sessionId))
        : new Set<string>(),
    [mutation.isPending, mutation.variables],
  );
  const isGenerating = mutation.isPending || activeMutationCount > 0;

  const notes = useMemo(
    () =>
      built.notes.map((note) => ({
        ...note,
        isGenerating: generatingIds.has(note.sessionId),
        isRegenerateDisabled: isGenerating,
      })),
    [built.notes, generatingIds, isGenerating],
  );

  const regenerate = useCallback(
    (targetSessionId: string) => {
      if (!enabled || !model || isGenerating) {
        return;
      }

      const request = built.requests.find(
        (request) => request.sessionId === targetSessionId,
      );
      if (!request) {
        return;
      }

      mutation.mutate([request]);
    },
    [built.requests, enabled, isGenerating, model, mutation],
  );

  const regenerateAll = useCallback(() => {
    if (!enabled || !model || built.requests.length === 0 || isGenerating) {
      return;
    }

    mutation.mutate(built.requests);
  }, [built.requests, enabled, isGenerating, model, mutation]);

  return {
    notes,
    hasPastNotes: notes.length > 0,
    isGenerating,
    canGenerate: enabled && Boolean(model),
    regenerate,
    regenerateAll,
  };
}

function usePastSessionNotesData(enabled: boolean): PastSessionNotesData {
  const { data: sessions = EMPTY_SESSIONS } = useLiveQuery<
    PastSessionRow,
    Record<string, PastSessionRow>
  >({
    sql: `
      SELECT
        id,
        owner_user_id AS user_id,
        title,
        created_at,
        event_json
      FROM sessions
      WHERE deleted_at IS NULL
      ORDER BY created_at, id
    `,
    enabled,
    mapRows: (rows) =>
      Object.fromEntries(rows.map((row) => [row.id, row])) as Record<
        string,
        PastSessionRow
      >,
  });
  const { data: participants = EMPTY_PARTICIPANTS } = useLiveQuery<
    PastParticipantRow,
    PastParticipantRow[]
  >({
    sql: `
      SELECT
        participant.session_id,
        participant.human_id,
        participant.owner_user_id AS user_id,
        participant.source,
        COALESCE(
          NULLIF(human.name, ''),
          NULLIF(participant.display_name, ''),
          participant.human_id
        ) AS name
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
      WHERE participant.deleted_at IS NULL
      ORDER BY participant.session_id, participant.created_at, participant.id
    `,
    enabled,
    mapRows: (rows) => rows,
  });
  const { data: enhancedNotes = EMPTY_ENHANCED_NOTES } = useLiveQuery<
    PastEnhancedNoteRow,
    PastEnhancedNoteRow[]
  >({
    sql: `
      SELECT
        session_id,
        body AS content,
        sort_order AS position
      FROM session_documents
      WHERE kind = 'enhanced_note' AND deleted_at IS NULL
      ORDER BY session_id, sort_order, created_at, id
    `,
    enabled,
    mapRows: (rows) => rows,
  });
  const { data: keyFacts = EMPTY_KEY_FACTS } = useLiveQuery<
    PastKeyFactsRow,
    Record<string, PastKeyFactsRow>
  >({
    sql: `
      SELECT
        session_id,
        created_by AS user_id,
        created_at,
        updated_at,
        body AS content,
        source_hash
      FROM session_documents
      WHERE kind = 'key_facts' AND deleted_at IS NULL
      ORDER BY updated_at, id
    `,
    enabled,
    mapRows: (rows) => {
      const result: Record<string, PastKeyFactsRow> = {};
      for (const row of rows) result[row.session_id] = row;
      return result;
    },
  });

  return { sessions, participants, enhancedNotes, keyFacts };
}

export function buildPastSessionNotes(
  data: PastSessionNotesData,
  sessionId: string,
  userId: string | null,
): {
  notes: PastSessionNote[];
  missing: PastSessionNoteRequest[];
  requests: PastSessionNoteRequest[];
} {
  const currentSession = data.sessions[sessionId];
  if (!currentSession) {
    return { notes: [], missing: [], requests: [] };
  }

  const currentParticipantIds = getSessionParticipantIds(
    data.participants,
    sessionId,
    userId,
  );
  const currentEvent = getSessionEvent(currentSession);
  const currentSeriesId = getRecurrenceSeriesId(currentEvent);
  const currentTitleKey = getSessionTitleKey(currentSession);
  if (!currentSeriesId && !currentTitleKey) {
    return { notes: [], missing: [], requests: [] };
  }

  const currentTimestamp = getSessionTimestamp(currentSession);
  const items: Array<{
    note: PastSessionNote & { dateMs: number };
    request: PastSessionNoteRequest;
    isMissing: boolean;
  }> = [];

  for (const candidateSession of Object.values(data.sessions)) {
    const candidateSessionId = candidateSession.id;
    if (candidateSessionId === sessionId) {
      continue;
    }

    const candidateTimestamp = getSessionTimestamp(candidateSession);
    if (
      currentTimestamp > 0 &&
      candidateTimestamp > 0 &&
      candidateTimestamp >= currentTimestamp
    ) {
      continue;
    }

    const candidateEvent = getSessionEvent(candidateSession);
    const candidateParticipantIds = getSessionParticipantIds(
      data.participants,
      candidateSessionId,
      userId,
    );
    if (
      !isRelatedPastSession({
        currentParticipantIds,
        currentSeriesId,
        currentTitleKey,
        candidateParticipantIds,
        candidateSeriesId: getRecurrenceSeriesId(candidateEvent),
        candidateTitleKey: getSessionTitleKey(candidateSession),
      })
    ) {
      continue;
    }

    const source = getSessionKeyFactsSource(
      data.enhancedNotes,
      candidateSessionId,
    );
    if (!source) {
      continue;
    }

    const title = getSessionTitle(candidateSession);
    const dateLabel = formatSessionDate(candidateSession);
    const sourceHash = createSourceHash([title, dateLabel, source].join("\n"));
    const saved = getSavedKeyFacts(
      data.keyFacts,
      candidateSessionId,
      sourceHash,
    );
    const ownerUserId = getSessionUserId(candidateSession, userId);
    const participantNames = getSessionParticipantNames(
      data.participants,
      new Set([...currentParticipantIds, ...candidateParticipantIds]),
    );
    const request = {
      sessionId: candidateSessionId,
      userId: ownerUserId,
      title,
      dateLabel,
      sourceText: source,
      sourceHash,
      participantNames,
    };

    items.push({
      note: {
        sessionId: candidateSessionId,
        title,
        dateLabel,
        participantNames,
        summary: saved,
        isGenerating: false,
        dateMs: candidateTimestamp,
      },
      request,
      isMissing: !saved,
    });
  }

  const selected = items
    .sort((a, b) => b.note.dateMs - a.note.dateMs)
    .slice(0, MAX_PAST_NOTES);

  return {
    notes: selected.map(({ note }) => {
      const { dateMs: _dateMs, ...rest } = note;
      return rest;
    }),
    missing: selected.flatMap((item) => (item.isMissing ? [item.request] : [])),
    requests: selected.map((item) => item.request),
  };
}

async function generateAndSavePastSessionNotes({
  model,
  requests,
}: {
  model: LanguageModel;
  requests: PastSessionNoteRequest[];
}) {
  const rows: Array<PastSessionNoteRequest & { content: string }> = [];

  for (const request of requests) {
    const content = await generatePastSessionKeyFacts({ model, request });
    if (content) {
      rows.push({ ...request, content });
    }
  }

  if (rows.length === 0) {
    return;
  }

  const now = new Date().toISOString();
  await enqueueDatabaseWrite("session-key-facts", async () => {
    await executeTransaction(buildSessionKeyFactsStatements(rows, now));
  });
}

export function buildSessionKeyFactsStatements(
  rows: Array<{
    sessionId: string;
    userId: string;
    content: string;
    sourceHash: string;
  }>,
  now: string,
): Array<{ sql: string; params: unknown[] }> {
  return rows.flatMap((row) => [
    {
      sql: `
        UPDATE session_documents
        SET
          body = ?,
          source_hash = ?,
          updated_by = ?,
          updated_at = ?,
          deleted_at = NULL
        WHERE session_id = ?
          AND kind = 'key_facts'
          AND deleted_at IS NULL
          AND EXISTS (
            SELECT 1
            FROM sessions
            WHERE id = ? AND deleted_at IS NULL
          )
      `,
      params: [
        row.content,
        row.sourceHash,
        row.userId,
        now,
        row.sessionId,
        row.sessionId,
      ],
    },
    {
      sql: `
        INSERT INTO session_documents (
          id, workspace_id, session_id, kind, template_id, title,
          body_format, body, source_hash, generation_metadata_json,
          sort_order, created_by, updated_by, created_at, updated_at,
          deleted_at
        )
        SELECT ?, '', ?, 'key_facts', '', 'Key facts', 'markdown', ?, ?,
          '{}', 0, ?, ?, ?, ?, NULL
        WHERE EXISTS (
          SELECT 1
          FROM sessions
          WHERE id = ? AND deleted_at IS NULL
        )
          AND NOT EXISTS (
            SELECT 1
            FROM session_documents
            WHERE session_id = ?
              AND kind = 'key_facts'
              AND deleted_at IS NULL
          )
        ON CONFLICT(id) DO UPDATE SET
          session_id = excluded.session_id,
          kind = excluded.kind,
          title = excluded.title,
          body_format = excluded.body_format,
          body = excluded.body,
          source_hash = excluded.source_hash,
          updated_by = excluded.updated_by,
          updated_at = excluded.updated_at,
          deleted_at = NULL
      `,
      params: [
        `${row.sessionId}:key_facts`,
        row.sessionId,
        row.content,
        row.sourceHash,
        row.userId,
        row.userId,
        now,
        now,
        row.sessionId,
        row.sessionId,
      ],
    },
  ]);
}

async function generatePastSessionKeyFacts({
  model,
  request,
}: {
  model: LanguageModel;
  request: PastSessionNoteRequest;
}): Promise<string> {
  const system = await renderJinja(systemPromptTemplate, {});
  const prompt = await renderJinja(userPromptTemplate, {
    session: {
      title: request.title,
      date_label: request.dateLabel,
      participant_names: request.participantNames,
    },
    summaries: request.sourceText,
  });

  const result = await generateText({
    model,
    ...deterministicGenerationSettings(model),
    system,
    prompt,
    output: Output.object({ schema: keyFactsSchema }),
    maxRetries: 2,
    maxOutputTokens: 400,
    timeout: { totalMs: KEY_FACTS_GENERATION_TIMEOUT_MS },
  });

  return normalizeFacts(result.output?.facts ?? []).join("\n");
}

type TemplateContext = Partial<{ [key: string]: JsonValue }>;

async function renderJinja(templateContent: string, ctx: TemplateContext) {
  const result = await templateCommands.renderCustom(templateContent, ctx);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

function normalizeFacts(facts: string[]): string[] {
  const seen = new Set<string>();
  const normalized: string[] = [];

  for (const fact of facts) {
    const text = fact
      .replace(/^[-*]\s+/, "")
      .replace(/^\d+[.)]\s+/, "")
      .replace(SPACE_REGEX, " ")
      .trim();
    const key = text.toLowerCase();
    if (!text || seen.has(key)) {
      continue;
    }

    seen.add(key);
    normalized.push(text);
  }

  return normalized.slice(0, MAX_KEY_FACTS);
}

function getSavedKeyFacts(
  keyFacts: Record<string, PastKeyFactsRow>,
  sessionId: string,
  sourceHash: string,
): string | null {
  const row = keyFacts[sessionId];
  if (
    !row ||
    row.session_id !== sessionId ||
    row.source_hash !== sourceHash ||
    !row.content?.trim()
  ) {
    return null;
  }

  return row.content.trim();
}

function getSessionKeyFactsSource(
  enhancedNotes: PastEnhancedNoteRow[],
  sessionId: string,
): string | null {
  const summaries = enhancedNotes.filter(
    (note) => note.session_id === sessionId && note.content.trim(),
  );

  summaries.sort((a, b) => a.position - b.position);
  const summaryText = cleanSourceText(
    summaries.map((note) => extractPlainText(note.content)).join("\n\n"),
  );
  return summaryText ? truncateAtWord(summaryText, MAX_SOURCE_LENGTH) : null;
}

function cleanSourceText(text: string): string {
  return text
    .replace(/!\[[^\]]*]\([^)]+\)/g, "")
    .replace(/\[([^\]]+)]\([^)]+\)/g, "$1")
    .replace(/[`*_~>#]/g, "")
    .replace(/(^|\s)([-+]|[0-9]+[.)])\s+/g, " ")
    .replace(SPACE_REGEX, " ")
    .trim();
}

function truncateAtWord(text: string, maxLength: number): string {
  if (text.length <= maxLength) {
    return text;
  }

  const slice = text.slice(0, maxLength + 1);
  const lastSpace = slice.lastIndexOf(" ");
  const end = lastSpace > maxLength * 0.6 ? lastSpace : maxLength;
  return `${slice.slice(0, end).trim()}...`;
}

function getSessionParticipantIds(
  participants: PastParticipantRow[],
  sessionId: string,
  userId: string | null,
): Set<string> {
  const participantIds = new Set<string>();

  for (const mapping of participants) {
    if (
      mapping.session_id !== sessionId ||
      mapping.source === "excluded" ||
      !mapping.human_id
    ) {
      continue;
    }

    const ownerUserId =
      typeof mapping.user_id === "string" && mapping.user_id.trim()
        ? mapping.user_id
        : null;
    const isCurrentUser =
      (userId && mapping.human_id === userId) ||
      (!userId && ownerUserId && mapping.human_id === ownerUserId);
    if (!isCurrentUser) {
      participantIds.add(mapping.human_id);
    }
  }

  return participantIds;
}

function getSessionParticipantNames(
  participants: PastParticipantRow[],
  participantIds: Set<string>,
): string[] {
  const namesByHumanId = new Map<string, string>();
  for (const participant of participants) {
    if (participant.name.trim()) {
      namesByHumanId.set(participant.human_id, participant.name.trim());
    }
  }
  const seen = new Set<string>();
  const names: string[] = [];

  for (const participantId of participantIds) {
    const name = namesByHumanId.get(participantId) || participantId;
    const key = name.toLowerCase();
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    names.push(name);
  }

  return names.sort((a, b) => a.localeCompare(b));
}

function isRelatedPastSession({
  currentParticipantIds,
  currentSeriesId,
  currentTitleKey,
  candidateParticipantIds,
  candidateSeriesId,
  candidateTitleKey,
}: {
  currentParticipantIds: Set<string>;
  currentSeriesId: string | null;
  currentTitleKey: string;
  candidateParticipantIds: Set<string>;
  candidateSeriesId: string | null;
  candidateTitleKey: string;
}) {
  if (currentSeriesId && candidateSeriesId === currentSeriesId) {
    return true;
  }

  if (!currentTitleKey || currentTitleKey !== candidateTitleKey) {
    return false;
  }

  if (currentParticipantIds.size === 0 || candidateParticipantIds.size === 0) {
    return true;
  }

  for (const participantId of candidateParticipantIds) {
    if (currentParticipantIds.has(participantId)) {
      return true;
    }
  }

  return false;
}

function getSessionTitle(session: { title?: string }): string {
  return session.title?.trim() || "Untitled";
}

function getSessionTitleKey(session: { title?: string }): string {
  const key = getSessionTitle(session).toLowerCase().replace(SPACE_REGEX, " ");
  return GENERIC_TITLE_KEYS.has(key) ? "" : key;
}

function getSessionUserId(
  session: { user_id?: string },
  fallbackUserId: string | null,
): string {
  return session.user_id?.trim() || fallbackUserId || "";
}

function getRecurrenceSeriesId(
  event: ReturnType<typeof getSessionEvent>,
): string | null {
  const seriesId = event?.recurrence_series_id?.trim();
  return seriesId || null;
}

function getSessionTimestamp(session: {
  created_at?: string;
  event_json?: string;
}): number {
  const event = getSessionEvent(session);
  const value = event?.started_at || session.created_at;
  if (!value) {
    return 0;
  }

  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? 0 : timestamp;
}

function formatSessionDate(session: {
  created_at?: string;
  event_json?: string;
}): string {
  const event = getSessionEvent(session);
  const parsed = safeParseDate(event?.started_at || session.created_at);
  return parsed ? format(parsed, "MMM d, yyyy") : "";
}

function createSourceHash(text: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16);
}
