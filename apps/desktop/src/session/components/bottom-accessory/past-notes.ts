import { useMutation } from "@tanstack/react-query";
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
import { extractPlainText } from "~/search/contexts/engine/utils";
import { getSessionEvent } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";

export type PastSessionNote = {
  sessionId: string;
  title: string;
  dateLabel: string;
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
};

export type PastSessionNotesResult = {
  notes: PastSessionNote[];
  hasPastNotes: boolean;
  isGenerating: boolean;
  canGenerate: boolean;
  generateMissing: () => void;
  regenerate: (sessionId: string) => void;
};

type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

const MAX_PAST_NOTES = 8;
const MAX_SOURCE_LENGTH = 6000;
const MAX_KEY_FACTS = 3;
const SPACE_REGEX = /\s+/g;
const GENERIC_TITLE_KEYS = new Set(["new note", "untitled"]);

const keyFactsSchema = z.object({
  facts: z.array(z.string()).min(1).max(MAX_KEY_FACTS),
});

export function usePastSessionNotes(
  sessionId: string,
  { enabled = true }: { enabled?: boolean } = {},
): PastSessionNotesResult {
  const store = main.UI.useStore(main.STORE_ID);
  const sessionsTable = main.UI.useTable(
    enabled ? "sessions" : ("__disabled_past_notes_sessions" as "sessions"),
    main.STORE_ID,
  );
  const participantsTable = main.UI.useTable(
    enabled
      ? "mapping_session_participant"
      : ("__disabled_past_notes_participants" as "mapping_session_participant"),
    main.STORE_ID,
  );
  const enhancedNotesTable = main.UI.useTable(
    enabled
      ? "enhanced_notes"
      : ("__disabled_past_notes_enhanced" as "enhanced_notes"),
    main.STORE_ID,
  );
  const keyFactsTable = main.UI.useTable(
    enabled
      ? "session_key_facts"
      : ("__disabled_past_notes_key_facts" as "session_key_facts"),
    main.STORE_ID,
  );
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const model = useLanguageModel("enhance");

  const built = useMemo(() => {
    if (!enabled || !store) {
      return { notes: [], missing: [], requests: [] };
    }

    return buildPastSessionNotes(
      store,
      sessionId,
      typeof userId === "string" ? userId : null,
    );
  }, [
    store,
    enabled,
    sessionId,
    userId,
    sessionsTable,
    participantsTable,
    enhancedNotesTable,
    keyFactsTable,
  ]);

  const mutation = useMutation({
    mutationFn: async (requests: PastSessionNoteRequest[]) => {
      if (!store || !model || requests.length === 0) {
        return;
      }

      await generateAndSavePastSessionNotes({
        store,
        model,
        requests,
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

  const notes = useMemo(
    () =>
      built.notes.map((note) => ({
        ...note,
        isGenerating: generatingIds.has(note.sessionId),
        isRegenerateDisabled: mutation.isPending,
      })),
    [built.notes, generatingIds, mutation.isPending],
  );

  const generateMissing = useCallback(() => {
    if (
      !enabled ||
      !model ||
      built.missing.length === 0 ||
      mutation.isPending
    ) {
      return;
    }

    void mutation.mutateAsync(built.missing);
  }, [built.missing, enabled, model, mutation]);

  const regenerate = useCallback(
    (targetSessionId: string) => {
      if (!enabled || !model || mutation.isPending) {
        return;
      }

      const request = built.requests.find(
        (request) => request.sessionId === targetSessionId,
      );
      if (!request) {
        return;
      }

      void mutation.mutateAsync([request]);
    },
    [built.requests, enabled, model, mutation],
  );

  return {
    notes,
    hasPastNotes: notes.length > 0,
    isGenerating: mutation.isPending,
    canGenerate: enabled && Boolean(model),
    generateMissing,
    regenerate,
  };
}

export function buildPastSessionNotes(
  store: MainStore,
  sessionId: string,
  userId: string | null,
): {
  notes: PastSessionNote[];
  missing: PastSessionNoteRequest[];
  requests: PastSessionNoteRequest[];
} {
  const currentSession = store.getRow("sessions", sessionId);
  if (!currentSession) {
    return { notes: [], missing: [], requests: [] };
  }

  const currentParticipantIds = getSessionParticipantIds(
    store,
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

  store.forEachRow("sessions", (candidateSessionId, _forEachCell) => {
    if (candidateSessionId === sessionId) {
      return;
    }

    const candidateSession = store.getRow("sessions", candidateSessionId);
    if (!candidateSession) {
      return;
    }

    const candidateTimestamp = getSessionTimestamp(candidateSession);
    if (
      currentTimestamp > 0 &&
      candidateTimestamp > 0 &&
      candidateTimestamp >= currentTimestamp
    ) {
      return;
    }

    const candidateEvent = getSessionEvent(candidateSession);
    const candidateParticipantIds = getSessionParticipantIds(
      store,
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
      return;
    }

    const source = getSessionKeyFactsSource(store, candidateSessionId);
    if (!source) {
      return;
    }

    const title = getSessionTitle(candidateSession);
    const dateLabel = formatSessionDate(candidateSession);
    const sourceHash = createSourceHash([title, dateLabel, source].join("\n"));
    const saved = getSavedKeyFacts(store, candidateSessionId, sourceHash);
    const ownerUserId = getSessionUserId(candidateSession, userId);
    const request = {
      sessionId: candidateSessionId,
      userId: ownerUserId,
      title,
      dateLabel,
      sourceText: source,
      sourceHash,
    };

    items.push({
      note: {
        sessionId: candidateSessionId,
        title,
        dateLabel,
        summary: saved,
        isGenerating: false,
        dateMs: candidateTimestamp,
      },
      request,
      isMissing: !saved,
    });
  });

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
  store,
  model,
  requests,
}: {
  store: MainStore;
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
  store.transaction(() => {
    for (const row of rows) {
      const existingCreatedAt = store.getCell(
        "session_key_facts",
        row.sessionId,
        "created_at",
      );

      store.setRow("session_key_facts", row.sessionId, {
        user_id: row.userId,
        session_id: row.sessionId,
        created_at:
          typeof existingCreatedAt === "string" && existingCreatedAt
            ? existingCreatedAt
            : now,
        updated_at: now,
        content: row.content,
        source_hash: row.sourceHash,
      });
    }
  });
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
    },
    notes: request.sourceText,
  });

  const result = await generateText({
    model,
    temperature: 0,
    system,
    prompt,
    output: Output.object({ schema: keyFactsSchema }),
    maxRetries: 2,
    maxOutputTokens: 400,
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
  store: MainStore,
  sessionId: string,
  sourceHash: string,
): string | null {
  const row = store.getRow("session_key_facts", sessionId);
  if (
    row.session_id !== sessionId ||
    row.source_hash !== sourceHash ||
    !row.content?.trim()
  ) {
    return null;
  }

  return row.content.trim();
}

function getSessionKeyFactsSource(
  store: MainStore,
  sessionId: string,
): string | null {
  const enhancedNotes: Array<{ content: string; position: number }> = [];

  store.forEachRow("enhanced_notes", (noteId, _forEachCell) => {
    const note = store.getRow("enhanced_notes", noteId);
    if (note.session_id !== sessionId || !note.content?.trim()) {
      return;
    }

    enhancedNotes.push({
      content: note.content,
      position: typeof note.position === "number" ? note.position : 0,
    });
  });

  enhancedNotes.sort((a, b) => a.position - b.position);
  const enhancedText = cleanSourceText(
    enhancedNotes.map((note) => extractPlainText(note.content)).join("\n\n"),
  );
  if (enhancedText) {
    return truncateAtWord(enhancedText, MAX_SOURCE_LENGTH);
  }

  const rawMd = store.getCell("sessions", sessionId, "raw_md");
  const rawText = cleanSourceText(extractPlainText(rawMd));
  return rawText ? truncateAtWord(rawText, MAX_SOURCE_LENGTH) : null;
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
  store: MainStore,
  sessionId: string,
  userId: string | null,
): Set<string> {
  const participantIds = new Set<string>();

  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mapping = store.getRow("mapping_session_participant", mappingId);
    if (
      mapping.session_id !== sessionId ||
      mapping.source === "excluded" ||
      !mapping.human_id
    ) {
      return;
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
  });

  return participantIds;
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
