import { useCallback, useMemo } from "react";

import type { SessionEvent } from "@hypr/store";

import { getSessionEvent } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";

export type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;
export type MainIndexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;

// --- store access (thin escape hatch for imperative helpers) --------------

export function useMainStore(): MainStore | undefined {
  return main.UI.useStore(main.STORE_ID);
}

export function useMainIndexes(): MainIndexes | undefined {
  return main.UI.useIndexes(main.STORE_ID);
}

export function useCurrentUserId(): string | undefined {
  return main.UI.useValue("user_id", main.STORE_ID) as string | undefined;
}

export function useAiLanguage(): string {
  return (
    (main.UI.useValue("ai_language", main.STORE_ID) as string | undefined) ??
    "en"
  );
}

// --- session row reads ----------------------------------------------------

type SessionStringField =
  | "title"
  | "raw_md"
  | "created_at"
  | "event_json"
  | "folder_id"
  | "user_id";

export function useSessionCell(
  sessionId: string,
  field: SessionStringField,
): string {
  const v = main.UI.useCell("sessions", sessionId, field, main.STORE_ID);
  return (v as string | undefined) ?? "";
}

// Some header.tsx paths read fields that aren't in the schema (returns
// undefined). Kept as a loose-typed helper so header migration can compile.
export function useSessionUntypedCell(
  sessionId: string,
  field: string,
): string | undefined {
  const cell = main.UI.useCell(
    "sessions",
    sessionId,
    field as "title",
    main.STORE_ID,
  );
  return cell as string | undefined;
}

export function useAllSessionIds(): string[] {
  return main.UI.useRowIds("sessions", main.STORE_ID);
}

export function useSession(sessionId: string) {
  const title = main.UI.useCell("sessions", sessionId, "title", main.STORE_ID);
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const createdAt = main.UI.useCell(
    "sessions",
    sessionId,
    "created_at",
    main.STORE_ID,
  );
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  const folderId = main.UI.useCell(
    "sessions",
    sessionId,
    "folder_id",
    main.STORE_ID,
  );

  const event = useMemo(
    () => getSessionEvent({ event_json: eventJson }),
    [eventJson],
  );

  return useMemo(
    () => ({ title, rawMd, createdAt, event, folderId }),
    [title, rawMd, createdAt, event, folderId],
  );
}

export function useSessionEvent(sessionId: string): SessionEvent | null {
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  return useMemo(() => getSessionEvent({ event_json: eventJson }), [eventJson]);
}

export function useSessionIdsInFolder(folderId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.sessionsByFolder,
    folderId,
    main.STORE_ID,
  );
}

export function useUpdateSessionCell(
  sessionId: string,
  field: SessionStringField,
): (value: string) => void {
  const store = useMainStore();
  return useCallback(
    (value) => {
      if (!store) return;
      store.setPartialRow("sessions", sessionId, {
        [field]: value,
      } as Record<string, string>);
    },
    [store, sessionId, field],
  );
}

export function useUpdateSessionRawMd(
  sessionId: string,
): (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => void {
  return main.UI.useSetPartialRowCallback(
    "sessions",
    sessionId,
    (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => ({
      raw_md: e.target.value,
    }),
    [],
    main.STORE_ID,
  );
}

export function useSetSessionCreatedAt(
  sessionId: string,
): (created_at: string) => void {
  return main.UI.useSetCellCallback(
    "sessions",
    sessionId,
    "created_at",
    (created_at: string) => created_at,
    [],
    main.STORE_ID,
  );
}

// --- participant reads / writes ------------------------------------------

export function useSessionParticipantMappingIds(sessionId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId,
    main.STORE_ID,
  );
}

export type ParticipantDetails = {
  mappingId: string;
  session_id: string;
  human_id: string;
  human_name?: string;
  human_email?: string;
  human_job_title?: string;
  human_linkedin_username?: string;
  org_id?: string;
  org_name?: string;
};

export function useSessionParticipantDetails(
  mappingId: string,
): ParticipantDetails | null {
  const row = main.UI.useResultRow(
    main.QUERIES.sessionParticipantsWithDetails,
    mappingId,
    main.STORE_ID,
  );
  return useMemo(() => {
    if (!row || Object.keys(row).length === 0) return null;
    return {
      mappingId,
      session_id: (row.session_id as string) ?? "",
      human_id: (row.human_id as string) ?? "",
      human_name: row.human_name as string | undefined,
      human_email: row.human_email as string | undefined,
      human_job_title: row.human_job_title as string | undefined,
      human_linkedin_username: row.human_linkedin_username as
        | string
        | undefined,
      org_id: row.org_id as string | undefined,
      org_name: row.org_name as string | undefined,
    };
  }, [mappingId, row]);
}

export function useParticipantSourceCell(mappingId: string): string {
  const v = main.UI.useCell(
    "mapping_session_participant",
    mappingId,
    "source",
    main.STORE_ID,
  );
  return (v as string | undefined) ?? "";
}

export function useAllHumanIds(): string[] {
  return main.UI.useRowIds("humans", main.STORE_ID);
}

export function useSessionParticipantMutations(): {
  addParticipant: (args: {
    sessionId: string;
    humanId: string;
    source: string;
  }) => string;
  deleteMapping: (mappingId: string) => void;
  updateMappingHumanId: (mappingId: string, humanId: string) => void;
  createHuman: (args: { id: string; name: string; email: string }) => void;
} {
  const store = useMainStore();

  const addParticipant = useCallback(
    ({
      sessionId,
      humanId,
      source,
    }: {
      sessionId: string;
      humanId: string;
      source: string;
    }) => {
      if (!store) return "";
      const userId = (store.getValue("user_id") as string | undefined) ?? "";
      if (!userId) return "";
      const mappingId = crypto.randomUUID();
      store.setRow("mapping_session_participant", mappingId, {
        user_id: userId,
        session_id: sessionId,
        human_id: humanId,
        source,
      });
      return mappingId;
    },
    [store],
  );

  const deleteMapping = useCallback(
    (mappingId: string) => {
      if (!store) return;
      store.delRow("mapping_session_participant", mappingId);
    },
    [store],
  );

  const updateMappingHumanId = useCallback(
    (mappingId: string, humanId: string) => {
      if (!store) return;
      store.setPartialRow("mapping_session_participant", mappingId, {
        human_id: humanId,
      });
    },
    [store],
  );

  const createHuman = useCallback(
    ({ id, name, email }: { id: string; name: string; email: string }) => {
      if (!store) return;
      const userId = (store.getValue("user_id") as string | undefined) ?? "";
      if (!userId) return;
      store.setRow("humans", id, {
        user_id: userId,
        created_at: new Date().toISOString(),
        name,
        email,
        org_id: "",
        job_title: "",
        linkedin_username: "",
        memo: "",
      });
    },
    [store],
  );

  return { addParticipant, deleteMapping, updateMappingHumanId, createHuman };
}

// --- tag reads / writes ---------------------------------------------------

export function useSessionTagMappingIds(sessionId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.tagSessionsBySession,
    sessionId,
    main.STORE_ID,
  );
}

export function useTagMappingCell(
  mappingId: string,
  field: "tag_id" | "session_id",
): string {
  const v = main.UI.useCell(
    "mapping_tag_session",
    mappingId,
    field,
    main.STORE_ID,
  );
  return (v as string | undefined) ?? "";
}

export function useTagName(tagId: string): string {
  const v = main.UI.useCell("tags", tagId, "name", main.STORE_ID);
  return (v as string | undefined) ?? "";
}

export function useSessionTagMutations(): {
  createTag: (args: { id: string; name: string }) => void;
  addTagToSession: (args: { tagId: string; sessionId: string }) => string;
  deleteTagMapping: (mappingId: string) => void;
} {
  const store = useMainStore();

  const createTag = useCallback(
    ({ id, name }: { id: string; name: string }) => {
      if (!store) return;
      const userId = (store.getValue("user_id") as string | undefined) ?? "";
      store.setRow("tags", id, { user_id: userId, name });
    },
    [store],
  );

  const addTagToSession = useCallback(
    ({ tagId, sessionId }: { tagId: string; sessionId: string }) => {
      if (!store) return "";
      const userId = (store.getValue("user_id") as string | undefined) ?? "";
      const mappingId = crypto.randomUUID();
      store.setRow("mapping_tag_session", mappingId, {
        user_id: userId,
        tag_id: tagId,
        session_id: sessionId,
      });
      return mappingId;
    },
    [store],
  );

  const deleteTagMapping = useCallback(
    (mappingId: string) => {
      if (!store) return;
      store.delRow("mapping_tag_session", mappingId);
    },
    [store],
  );

  return { createTag, addTagToSession, deleteTagMapping };
}

// --- transcript reads -----------------------------------------------------

export function useTranscriptIdsForSession(sessionId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );
}

export function useTranscriptsTable() {
  return main.UI.useTable("transcripts", main.STORE_ID);
}

export function useTranscriptSessionId(transcriptId: string): string {
  const v = main.UI.useCell(
    "transcripts",
    transcriptId,
    "session_id",
    main.STORE_ID,
  );
  return (v as string | undefined) ?? "";
}

export function useParticipantMappingsTable() {
  return main.UI.useTable("mapping_session_participant", main.STORE_ID);
}

export function useHumansTable() {
  return main.UI.useTable("humans", main.STORE_ID);
}

// --- enhanced-note cell reads/writes --------------------------------------

export function useEnhancedNoteCell(
  enhancedNoteId: string,
  field: "title" | "content" | "template_id" | "position",
): string {
  const v = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    field,
    main.STORE_ID,
  );
  return (v as string | undefined) ?? "";
}

// `error` is read off enhanced_notes in some paths but isn't in the
// schema; treat it as an untyped escape hatch.
export function useEnhancedNoteUntypedCell(
  enhancedNoteId: string,
  field: string,
): string | undefined {
  const v = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    field as "title",
    main.STORE_ID,
  );
  return v as string | undefined;
}

export function useUpdateEnhancedNoteContent(
  enhancedNoteId: string,
): (content: string) => void {
  return main.UI.useSetPartialRowCallback(
    "enhanced_notes",
    enhancedNoteId,
    (content: string) => ({ content }),
    [],
    main.STORE_ID,
  );
}

// --- daily notes ---------------------------------------------------------

export function useUpdateDailyNoteContent(
  date: string,
): (contentJson: string) => void {
  const store = useMainStore();
  return useCallback(
    (contentJson) => {
      if (!store) return;
      store.setPartialRow("daily_notes", date, {
        content: contentJson,
        date,
      });
    },
    [store, date],
  );
}

// --- export queries -------------------------------------------------------

export function useExportTimelineSessions() {
  return main.UI.useResultTable(main.QUERIES.timelineSessions, main.STORE_ID);
}

export function useExportVisibleHumans() {
  return main.UI.useResultTable(main.QUERIES.visibleHumans, main.STORE_ID);
}

export function useExportVisibleOrganizations() {
  return main.UI.useResultTable(
    main.QUERIES.visibleOrganizations,
    main.STORE_ID,
  );
}

export function useExportSessionParticipantsTable() {
  return main.UI.useResultTable(
    main.QUERIES.sessionParticipantsWithDetails,
    main.STORE_ID,
  );
}

export function useTimelineEventsTable() {
  return main.UI.useResultTable(main.QUERIES.timelineEvents, main.STORE_ID);
}

export function useTimelineSessionsTable() {
  return main.UI.useResultTable(main.QUERIES.timelineSessions, main.STORE_ID);
}

// Literal names mirroring `~/store/tinybase/store/main` QUERIES/INDEXES.
// Inlined so tests can mock main without having to mirror every constant.
export const SESSION_PARTICIPANTS_WITH_DETAILS_QUERY =
  "sessionParticipantsWithDetails";
export const TIMELINE_EVENTS_QUERY = "timelineEvents";
export const TIMELINE_SESSIONS_QUERY = "timelineSessions";
export const TRANSCRIPT_BY_SESSION_INDEX = "transcriptBySession";
export const ENHANCED_NOTES_BY_SESSION_INDEX = "enhancedNotesBySession";
export const SESSION_PARTICIPANTS_BY_SESSION_INDEX =
  "sessionParticipantsBySession";
export const SESSIONS_BY_FOLDER_INDEX = "sessionsByFolder";

// --- queries / indexes handles (for direct imperative use) ---------------

export function useMainQueries() {
  return main.UI.useQueries(main.STORE_ID);
}
