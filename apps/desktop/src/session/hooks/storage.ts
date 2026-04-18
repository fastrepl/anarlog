import { useCallback, useMemo } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import type { RenderTranscriptRequest } from "@hypr/plugin-transcription";
import type { SessionEvent } from "@hypr/store";

import type { ContextEntity, ContextRef } from "~/chat/context/entities";
import { hydrateSessionContextFromFs } from "~/chat/context/session-context-hydrator";
import type { ResolvedChatContext } from "~/chat/transport";
import { json2md, md2json, parseJsonContent } from "~/editor/markdown";
import { getSessionEvent } from "~/session/utils";
import { useSessionTabLifecycle as useSharedSessionTabLifecycle } from "~/shared/desktop-tab-lifecycle";
import {
  captureSessionData,
  deleteSessionCascade,
  restoreSessionData,
} from "~/store/tinybase/store/deleteSession";
import { importData } from "~/store/tinybase/store/importer";
import * as main from "~/store/tinybase/store/main";
import { save } from "~/store/tinybase/store/save";
import {
  createSession,
  getOrCreateSessionForEventId,
} from "~/store/tinybase/store/sessions";
import { buildRenderTranscriptRequestFromStore } from "~/stt/render-transcript";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";
import {
  parseTranscriptWords,
  parseTranscriptHints,
  updateTranscriptHints,
  upsertSpeakerAssignment,
} from "~/stt/utils";

// Storage boundary for session-domain consumers. Keep exported types/hooks
// storage-agnostic so internals can move from TinyBase to SQLite without
// changing call sites.
export type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;
export type MainIndexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;
export type SliceIndexReader = Pick<MainIndexes, "getSliceRowIds">;
export type SessionListItem = {
  id: string;
  title: string;
  createdAt: string;
};
export type SessionFolderTree = {
  topLevel: string[];
  byParent: Record<string, string[]>;
};

// --- store access (thin escape hatch for imperative helpers) --------------

export function useMainStore(): MainStore | undefined {
  return main.UI.useStore(main.STORE_ID);
}

export function useMainIndexes(): MainIndexes | undefined {
  return main.UI.useIndexes(main.STORE_ID);
}

export function useCalendarSyncRuntime(): {
  store: MainStore | undefined;
  queries: ReturnType<typeof main.UI.useQueries>;
} {
  return {
    store: useMainStore(),
    queries: main.UI.useQueries(main.STORE_ID),
  };
}

export function useSessionTabLifecycle(args: {
  onEmpty?: (() => void) | null;
  onZeroTabs?: (() => void) | null;
}) {
  const store = useMainStore();
  const indexes = useMainIndexes();

  useSharedSessionTabLifecycle({
    store,
    indexes,
    onEmpty: args.onEmpty,
    onZeroTabs: args.onZeroTabs,
  });
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
  return useSessionCellOrEmpty(sessionId, field);
}

export function useSessionCellOrEmpty(
  sessionId: string,
  field: SessionStringField,
): string {
  const v = main.UI.useCell("sessions", sessionId, field, main.STORE_ID);
  return (v as string | undefined) ?? "";
}

export function useSessionCellOptional(
  sessionId: string,
  field: SessionStringField,
): string | undefined {
  const v = main.UI.useCell("sessions", sessionId, field, main.STORE_ID);
  return v as string | undefined;
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

export function useSessionTitleField(sessionId: string): {
  value: string;
  setValue: (value: string) => void;
} {
  const value = useSessionCell(sessionId, "title");
  const setValue = useUpdateSessionCell(sessionId, "title");
  return { value, setValue };
}

export function useOpenNoteSessions(): SessionListItem[] {
  const sessionIds = useAllSessionIds();
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);

  return useMemo(() => {
    return sessionIds
      .map((id) => {
        const session = sessionsTable[id];
        return {
          id,
          title:
            typeof session?.title === "string" && session.title
              ? session.title
              : "Untitled",
          createdAt:
            typeof session?.created_at === "string" ? session.created_at : "",
        } satisfies SessionListItem;
      })
      .sort((a, b) => {
        if (!a.createdAt || !b.createdAt) return 0;
        return (
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
        );
      });
  }, [sessionIds, sessionsTable]);
}

export function useSessionFolderTree(): SessionFolderTree {
  const sessionIds = useAllSessionIds();
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);

  return useMemo(() => {
    const allFolders = new Set<string>();

    for (const id of sessionIds) {
      const folderId = sessionsTable[id]?.folder_id;
      if (typeof folderId !== "string" || !folderId) continue;

      const parts = folderId.split("/");
      for (let i = 1; i <= parts.length; i++) {
        allFolders.add(parts.slice(0, i).join("/"));
      }
    }

    const topLevel: string[] = [];
    const byParent: Record<string, string[]> = {};

    for (const folder of allFolders) {
      const parts = folder.split("/");
      if (parts.length === 1) {
        topLevel.push(folder);
      } else {
        const parent = parts.slice(0, -1).join("/");
        byParent[parent] = byParent[parent] || [];
        byParent[parent].push(folder);
      }
    }

    return {
      topLevel: topLevel.sort(),
      byParent: Object.fromEntries(
        Object.entries(byParent).map(([key, value]) => [key, value.sort()]),
      ),
    };
  }, [sessionIds, sessionsTable]);
}

export function useEditTabTitles(
  sessionId: string,
  enhancedNoteId: string,
): {
  sessionTitle: string | null;
  summaryTitle: string | null;
} {
  const sessionTitle = useSessionCellOptional(sessionId, "title");
  const summaryTitle = useEnhancedNoteCell(enhancedNoteId, "title");

  return useMemo(
    () => ({
      sessionTitle:
        typeof sessionTitle === "string" && sessionTitle.trim()
          ? sessionTitle
          : null,
      summaryTitle:
        typeof summaryTitle === "string" && summaryTitle.trim()
          ? summaryTitle
          : null,
    }),
    [sessionTitle, summaryTitle],
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

export type SessionParticipantSummary = {
  mappingId: string;
  sessionId: string;
  humanId: string;
  source: string | undefined;
  humanName: string | undefined;
  humanEmail: string | undefined;
  humanJobTitle: string | undefined;
  humanLinkedinUsername: string | undefined;
  orgId: string | undefined;
  orgName: string | undefined;
};

type HumanSearchCandidate = {
  id: string;
  name: string;
  email: string;
  orgId: string | undefined;
  jobTitle: string | undefined;
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

export function useSessionParticipants(
  sessionId: string,
): SessionParticipantSummary[] {
  const mappingIds = useSessionParticipantMappingIds(sessionId);
  const mappingTable = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const participantRows = main.UI.useResultTable(
    main.QUERIES.sessionParticipantsWithDetails,
    main.STORE_ID,
  );

  return useMemo(() => {
    return mappingIds
      .map((mappingId) => {
        const details = participantRows[mappingId];
        if (!details) return null;

        const humanId = details.human_id as string | undefined;
        const mappedSessionId = details.session_id as string | undefined;
        if (!humanId || mappedSessionId !== sessionId) return null;

        const mappingRow = mappingTable[mappingId];
        return {
          mappingId,
          sessionId: mappedSessionId,
          humanId,
          source: mappingRow?.source as string | undefined,
          humanName: details.human_name as string | undefined,
          humanEmail: details.human_email as string | undefined,
          humanJobTitle: details.human_job_title as string | undefined,
          humanLinkedinUsername: details.human_linkedin_username as
            | string
            | undefined,
          orgId: details.org_id as string | undefined,
          orgName: details.org_name as string | undefined,
        } satisfies SessionParticipantSummary;
      })
      .filter((row): row is SessionParticipantSummary => row !== null);
  }, [mappingIds, mappingTable, participantRows, sessionId]);
}

export function useSessionParticipantHumanIds(sessionId: string): string[] {
  const participants = useSessionParticipants(sessionId);
  return useMemo(
    () =>
      participants
        .filter((participant) => participant.source !== "excluded")
        .map((participant) => participant.humanId),
    [participants],
  );
}

export function useSessionParticipantNames(sessionId: string): string[] {
  const participants = useSessionParticipants(sessionId);
  return useMemo(
    () =>
      participants
        .filter((participant) => participant.source !== "excluded")
        .map((participant) => participant.humanName?.trim() ?? "")
        .filter(Boolean),
    [participants],
  );
}

export function useSessionParticipantPeople(
  sessionId: string,
): { id: string; name: string }[] {
  const participants = useSessionParticipants(sessionId);
  return useMemo(() => {
    return participants
      .filter((participant) => participant.source !== "excluded")
      .map((participant) => ({
        id: participant.humanId,
        name: participant.humanName?.trim() || "",
      }))
      .filter((participant) => participant.id.length > 0);
  }, [participants]);
}

export function useSearchableHumans(
  inputValue: string,
  excludedHumanIds: Set<string>,
): HumanSearchCandidate[] {
  const humans = main.UI.useTable("humans", main.STORE_ID);

  return useMemo(() => {
    const searchLower = inputValue.trim().toLowerCase();

    return Object.entries(humans)
      .filter(([humanId]) => !excludedHumanIds.has(humanId))
      .map(([humanId, row]) => {
        const name = String(row.name ?? "");
        const email = String(row.email ?? "");

        if (
          searchLower &&
          !name.toLowerCase().includes(searchLower) &&
          !email.toLowerCase().includes(searchLower)
        ) {
          return null;
        }

        return {
          id: humanId,
          name,
          email,
          orgId:
            typeof row.org_id === "string" && row.org_id
              ? row.org_id
              : undefined,
          jobTitle:
            typeof row.job_title === "string" && row.job_title
              ? row.job_title
              : undefined,
        } satisfies HumanSearchCandidate;
      })
      .filter((human): human is HumanSearchCandidate => human !== null);
  }, [humans, inputValue, excludedHumanIds]);
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

function parseHumanIdFromHintValue(value: unknown): string | undefined {
  let data = value;
  if (typeof value === "string") {
    try {
      data = JSON.parse(value);
    } catch {
      return undefined;
    }
  }

  if (data && typeof data === "object" && "human_id" in data) {
    const humanId = (data as Record<string, unknown>).human_id;
    return typeof humanId === "string" ? humanId : undefined;
  }

  return undefined;
}

export function useRemoveSessionParticipant(): (args: {
  mappingId: string;
  assignedHumanId?: string;
  sessionId?: string;
  source?: string;
}) => void {
  const store = useMainStore();
  const indexes = useMainIndexes();

  return useCallback(
    ({ mappingId, assignedHumanId, sessionId, source }) => {
      if (!store) {
        return;
      }

      if (assignedHumanId && sessionId && indexes) {
        const transcriptIds = indexes.getSliceRowIds(
          main.INDEXES.transcriptBySession,
          sessionId,
        );

        for (const transcriptId of transcriptIds) {
          const hints = parseTranscriptHints(store, transcriptId);
          if (hints.length === 0) continue;

          const filteredHints = hints.filter((hint) => {
            if (hint.type !== "user_speaker_assignment") {
              return true;
            }
            const hintHumanId = parseHumanIdFromHintValue(hint.value);
            return hintHumanId !== assignedHumanId;
          });

          if (filteredHints.length !== hints.length) {
            updateTranscriptHints(store, transcriptId, filteredHints);
          }
        }
      }

      if (source === "auto") {
        store.setPartialRow("mapping_session_participant", mappingId, {
          source: "excluded",
        });
      } else {
        store.delRow("mapping_session_participant", mappingId);
      }
    },
    [indexes, store],
  );
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

export function useSessionTagNameMap(sessionId: string): Map<string, string> {
  const mappingIds = useSessionTagMappingIds(sessionId);
  const mappingTable = main.UI.useTable("mapping_tag_session", main.STORE_ID);
  const tagsTable = main.UI.useTable("tags", main.STORE_ID);

  return useMemo(() => {
    const byName = new Map<string, string>();

    for (const mappingId of mappingIds) {
      const tagId = mappingTable[mappingId]?.tag_id;
      if (typeof tagId !== "string" || !tagId) continue;

      const tagName = tagsTable[tagId]?.name;
      if (typeof tagName !== "string") continue;

      byName.set(tagName.toLowerCase(), tagId);
    }

    return byName;
  }, [mappingIds, mappingTable, tagsTable]);
}

export function useAddSessionTag(sessionId: string): (name: string) => void {
  const existingTagIdsByName = useSessionTagNameMap(sessionId);
  const store = useMainStore();

  return useCallback(
    (name: string) => {
      if (!store) return;

      const userId = store.getValue("user_id") as string | undefined;
      if (!userId) return;

      const normalized = name.toLowerCase();
      const existingTagId = existingTagIdsByName.get(normalized);

      let tagId = existingTagId;
      if (!tagId) {
        let foundTagId: string | null = null;
        store.forEachRow("tags", (rowId, _forEachCell) => {
          if (foundTagId) return;

          const tagName = store.getCell("tags", rowId, "name");
          if (
            typeof tagName === "string" &&
            tagName.toLowerCase() === normalized
          ) {
            foundTagId = rowId;
          }
        });

        tagId = foundTagId ?? crypto.randomUUID();
        if (!foundTagId) {
          store.setRow("tags", tagId, {
            user_id: userId,
            name,
          });
        }
      }

      let hasMapping = false;
      store.forEachRow("mapping_tag_session", (mappingId, _forEachCell) => {
        if (hasMapping) return;
        const existingSessionId = store.getCell(
          "mapping_tag_session",
          mappingId,
          "session_id",
        );
        const existingMappedTagId = store.getCell(
          "mapping_tag_session",
          mappingId,
          "tag_id",
        );
        hasMapping =
          existingSessionId === sessionId && existingMappedTagId === tagId;
      });

      if (hasMapping) return;

      const mappingId = crypto.randomUUID();
      store.setRow("mapping_tag_session", mappingId, {
        user_id: userId,
        tag_id: tagId,
        session_id: sessionId,
      });
    },
    [existingTagIdsByName, sessionId, store],
  );
}

// --- transcript reads -----------------------------------------------------

export function useTranscriptIdsForSession(sessionId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );
}

export function useTranscriptTimeRange(sessionId: string): {
  startedAt: number | null;
  endedAt: number | null;
} {
  const transcriptIds = useTranscriptIdsForSession(sessionId);
  const transcripts = main.UI.useTable("transcripts", main.STORE_ID);

  return useMemo(() => {
    let minStartedAt: number | null = null;
    let maxEndedAt: number | null = null;

    for (const transcriptId of transcriptIds) {
      const transcript = transcripts[transcriptId];
      if (!transcript) continue;

      const startedAt = transcript.started_at;
      const endedAt = transcript.ended_at;

      if (typeof startedAt === "number") {
        minStartedAt =
          minStartedAt === null ? startedAt : Math.min(minStartedAt, startedAt);
      }

      if (typeof endedAt === "number") {
        maxEndedAt =
          maxEndedAt === null ? endedAt : Math.max(maxEndedAt, endedAt);
      }
    }

    return { startedAt: minStartedAt, endedAt: maxEndedAt };
  }, [transcriptIds, transcripts]);
}

export function useTranscriptHasWords(sessionId: string): boolean {
  const transcriptIds = useTranscriptIdsForSession(sessionId);
  const transcriptsTable = useTranscriptsTable();
  const store = useMainStore();

  return useMemo(() => {
    if (!store) return false;
    return transcriptIds.some(
      (transcriptId) => parseTranscriptWords(store, transcriptId).length > 0,
    );
  }, [store, transcriptIds, transcriptsTable]);
}

export function useTranscriptRenderRequest(
  transcriptIds: string[],
): RenderTranscriptRequest | null {
  const store = useMainStore();
  const transcriptsTable = useTranscriptsTable();
  const participantMappingsTable = useParticipantMappingsTable();
  const humansTable = useHumansTable();
  const selfHumanId = useCurrentUserId();

  return useMemo(() => {
    if (!store || transcriptIds.length === 0) {
      return null;
    }
    return buildRenderTranscriptRequestFromStore(store, transcriptIds);
  }, [
    store,
    transcriptIds,
    transcriptsTable,
    participantMappingsTable,
    humansTable,
    selfHumanId,
  ]);
}

export function useTranscriptOffsetMs(transcriptId: string): number {
  const transcriptStartedAt = main.UI.useCell(
    "transcripts",
    transcriptId,
    "started_at",
    main.STORE_ID,
  );
  const sessionId = useTranscriptSessionId(transcriptId);
  const transcriptIds = useTranscriptIdsForSession(sessionId);
  const transcriptsTable = useTranscriptsTable();

  return useMemo(() => {
    if (typeof transcriptStartedAt !== "number") {
      return 0;
    }

    let earliestStartedAt = Number.POSITIVE_INFINITY;
    for (const currentTranscriptId of transcriptIds ?? []) {
      const startedAt = transcriptsTable[currentTranscriptId]?.started_at;
      if (typeof startedAt === "number" && startedAt < earliestStartedAt) {
        earliestStartedAt = startedAt;
      }
    }

    return Number.isFinite(earliestStartedAt)
      ? transcriptStartedAt - earliestStartedAt
      : 0;
  }, [transcriptIds, transcriptStartedAt, transcriptsTable]);
}

export function useTranscriptLabelContext() {
  const store = useMainStore();
  return useMemo(
    () => (store ? defaultRenderLabelContext(store) : undefined),
    [store],
  );
}

export function useTranscriptSpeakerLabelManager(
  segments: Parameters<typeof SpeakerLabelManager.fromSegments>[0],
): SpeakerLabelManager {
  const labelContext = useTranscriptLabelContext();
  return useMemo(() => {
    if (!labelContext) {
      return new SpeakerLabelManager();
    }
    return SpeakerLabelManager.fromSegments(segments, labelContext);
  }, [labelContext, segments]);
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

export function useAssignTranscriptSpeaker(): (args: {
  transcriptId: string;
  segmentKey: unknown;
  humanId: string;
  anchorWordId: string;
}) => void {
  const store = useMainStore();

  return useCallback(
    ({ transcriptId, segmentKey, humanId, anchorWordId }) => {
      if (!store) return;

      upsertSpeakerAssignment(
        store,
        transcriptId,
        segmentKey as Parameters<typeof upsertSpeakerAssignment>[2],
        humanId,
        anchorWordId,
      );
    },
    [store],
  );
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

export function useDeleteEnhancedNote(): (enhancedNoteId: string) => void {
  const store = useMainStore();

  return useCallback(
    (enhancedNoteId: string) => {
      if (!store) return;
      store.delRow("enhanced_notes", enhancedNoteId);
    },
    [store],
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

export type ContactSearchResult = {
  id: string;
  name: string;
  email: string | null;
  jobTitle: string | null;
  organization: string | null;
  memo: string | null;
};

export type CalendarEventSearchResult = {
  id: string;
  title: string;
  startedAt: string | null;
  endedAt: string | null;
  location: string | null;
  meetingLink: string | null;
  description: string | null;
  participantCount: number;
  linkedSessionId: string | null;
};

export type SummaryEditCandidate = {
  enhancedNoteId: string;
  title: string;
  templateId?: string;
  position?: number;
};

export function useContactSearchIndex(): (
  query: string,
  limit: number,
) => Promise<ContactSearchResult[]> {
  const humansTable = main.UI.useTable("humans", main.STORE_ID);
  const organizationsTable = main.UI.useTable("organizations", main.STORE_ID);

  return useCallback(
    async (query: string, limit: number) => {
      const q = query.trim().toLowerCase();
      const rows = Object.entries(humansTable)
        .map(([id, row]) => {
          const orgId =
            typeof row.org_id === "string" && row.org_id ? row.org_id : null;
          const orgName =
            orgId &&
            typeof organizationsTable[orgId]?.name === "string" &&
            organizationsTable[orgId]?.name
              ? String(organizationsTable[orgId]?.name)
              : null;

          const name = typeof row.name === "string" ? row.name : "";
          const email =
            typeof row.email === "string" && row.email ? row.email : null;
          const jobTitle =
            typeof row.job_title === "string" && row.job_title
              ? row.job_title
              : null;
          const memo =
            typeof row.memo === "string" && row.memo ? row.memo : null;

          const searchable = [name, email, jobTitle, memo, orgName]
            .filter(Boolean)
            .join("\n")
            .toLowerCase();

          if (q && !searchable.includes(q)) {
            return null;
          }

          return {
            id,
            name,
            email,
            jobTitle,
            organization: orgName,
            memo,
            createdAt: Date.parse((row.created_at as string) || "") || 0,
          };
        })
        .filter(
          (
            row,
          ): row is ContactSearchResult & {
            createdAt: number;
          } => row !== null,
        )
        .sort((a, b) => b.createdAt - a.createdAt)
        .slice(0, limit)
        .map(({ createdAt: _createdAt, ...row }) => row);

      return rows;
    },
    [humansTable, organizationsTable],
  );
}

export function useCalendarEventSearchIndex(): (
  query: string,
  limit: number,
) => Promise<CalendarEventSearchResult[]> {
  const eventsTable = main.UI.useTable("events", main.STORE_ID);
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);

  return useCallback(
    async (query: string, limit: number) => {
      const q = query.trim().toLowerCase();
      const sessionByTrackingId = new Map<string, string>();

      for (const [sessionId, row] of Object.entries(sessionsTable)) {
        const event = getSessionEvent({
          event_json:
            typeof row.event_json === "string" ? row.event_json : undefined,
        });
        if (event?.tracking_id) {
          sessionByTrackingId.set(event.tracking_id, sessionId);
        }
      }

      return Object.entries(eventsTable)
        .map(([eventId, row]) => {
          const title = typeof row.title === "string" ? row.title : "";
          const startedAt =
            typeof row.started_at === "string" && row.started_at
              ? row.started_at
              : null;
          const endedAt =
            typeof row.ended_at === "string" && row.ended_at
              ? row.ended_at
              : null;
          const location =
            typeof row.location === "string" && row.location
              ? row.location
              : null;
          const meetingLink =
            typeof row.meeting_link === "string" && row.meeting_link
              ? row.meeting_link
              : null;
          const description =
            typeof row.description === "string" && row.description
              ? row.description
              : null;
          const trackingId =
            typeof row.tracking_id_event === "string"
              ? row.tracking_id_event
              : "";

          let participantCount = 0;
          if (
            typeof row.participants_json === "string" &&
            row.participants_json
          ) {
            try {
              const parsed = JSON.parse(row.participants_json);
              if (Array.isArray(parsed)) {
                participantCount = parsed.length;
              }
            } catch {}
          }

          const searchable = [title, location, meetingLink, description]
            .filter(Boolean)
            .join("\n")
            .toLowerCase();

          if (q && !searchable.includes(q)) {
            return null;
          }

          return {
            id: eventId,
            title: title || "Untitled event",
            startedAt,
            endedAt,
            location,
            meetingLink,
            description,
            participantCount,
            linkedSessionId: sessionByTrackingId.get(trackingId) ?? null,
            startedAtMs: startedAt ? Date.parse(startedAt) || 0 : 0,
          };
        })
        .filter(
          (
            row,
          ): row is CalendarEventSearchResult & {
            startedAtMs: number;
          } => row !== null,
        )
        .sort((a, b) => b.startedAtMs - a.startedAtMs)
        .slice(0, limit)
        .map(({ startedAtMs: _startedAtMs, ...row }) => row);
    },
    [eventsTable, sessionsTable],
  );
}

export function useSessionSearchTimestampLookup(): (
  sessionId: string,
) => number | undefined {
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);

  return useCallback(
    (sessionId: string) => {
      const createdAt = sessionsTable[sessionId]?.created_at;
      if (typeof createdAt !== "string" || !createdAt) {
        return undefined;
      }
      const parsed = Date.parse(createdAt);
      return Number.isNaN(parsed) ? undefined : parsed;
    },
    [sessionsTable],
  );
}

export function useSummaryEditRuntime(): {
  getSummaryCandidates: (sessionId: string) => SummaryEditCandidate[];
  getSummaryMarkdown: (enhancedNoteId: string) => string;
  applySummaryMarkdown: (enhancedNoteId: string, markdown: string) => void;
} {
  const enhancedNotes = main.UI.useTable("enhanced_notes", main.STORE_ID);
  const indexes = useMainIndexes();
  const store = useMainStore();

  const getSummaryCandidates = useCallback(
    (sessionId: string) => {
      if (!indexes) return [];
      return listEnhancedNoteIdsBySession(indexes, sessionId).map(
        (enhancedNoteId) => {
          const row = enhancedNotes[enhancedNoteId];
          return {
            enhancedNoteId,
            title:
              typeof row?.title === "string" && row.title.trim()
                ? row.title
                : "Summary",
            templateId:
              typeof row?.template_id === "string" && row.template_id
                ? row.template_id
                : undefined,
            position:
              typeof row?.position === "number" ? row.position : undefined,
          } satisfies SummaryEditCandidate;
        },
      );
    },
    [enhancedNotes, indexes],
  );

  const getSummaryMarkdown = useCallback(
    (enhancedNoteId: string) => {
      const raw = enhancedNotes[enhancedNoteId]?.content;
      return json2md(
        parseJsonContent(typeof raw === "string" ? raw : undefined),
      );
    },
    [enhancedNotes],
  );

  const applySummaryMarkdown = useCallback(
    (enhancedNoteId: string, markdown: string) => {
      if (!store) {
        throw new Error("Summary storage unavailable");
      }
      const json = md2json(markdown);
      store.setPartialRow("enhanced_notes", enhancedNoteId, {
        content: JSON.stringify(json),
      });
    },
    [store],
  );

  return { getSummaryCandidates, getSummaryMarkdown, applySummaryMarkdown };
}

export function useDisplayEntityRenderer(): (
  ref: ContextRef,
  removable: boolean,
) => ContextEntity {
  const sessionsTable = main.UI.useTable("sessions", main.STORE_ID);
  const humansTable = main.UI.useTable("humans", main.STORE_ID);
  const organizationsTable = main.UI.useTable("organizations", main.STORE_ID);

  return useCallback(
    (ref: ContextRef, removable: boolean): ContextEntity => {
      if (ref.kind === "session") {
        const row = sessionsTable[ref.sessionId];
        return {
          ...ref,
          title:
            typeof row?.title === "string" && row.title.trim()
              ? row.title
              : null,
          date:
            typeof row?.created_at === "string" && row.created_at.trim()
              ? row.created_at
              : null,
          removable,
        };
      }

      if (ref.kind === "human") {
        const row = humansTable[ref.humanId];
        const orgId = typeof row?.org_id === "string" ? row.org_id : null;
        const organization =
          orgId && organizationsTable[orgId] ? organizationsTable[orgId] : {};
        return {
          ...ref,
          name:
            typeof row?.name === "string" && row.name.trim() ? row.name : null,
          email:
            typeof row?.email === "string" && row.email.trim()
              ? row.email
              : null,
          organizationName:
            typeof organization.name === "string" && organization.name.trim()
              ? organization.name
              : null,
          removable,
        };
      }

      const row = organizationsTable[ref.organizationId];
      return {
        ...ref,
        name:
          typeof row?.name === "string" && row.name.trim() ? row.name : null,
        removable,
      };
    },
    [humansTable, organizationsTable, sessionsTable],
  );
}

export function useResolveContextRef(): (
  ref: ContextRef,
) => Promise<ResolvedChatContext | null> {
  const store = useMainStore();

  return useCallback(
    async (ref: ContextRef): Promise<ResolvedChatContext | null> => {
      if (!store) return null;

      if (ref.kind === "session") {
        const context = await hydrateSessionContextFromFs(
          store as Parameters<typeof hydrateSessionContextFromFs>[0],
          ref.sessionId,
        );
        return context
          ? ({ kind: "session", context } satisfies ResolvedChatContext)
          : null;
      }

      if (ref.kind === "human") {
        const human = store.getRow("humans", ref.humanId);
        const orgId = typeof human.org_id === "string" ? human.org_id : "";
        const organization =
          orgId && store.hasRow("organizations", orgId)
            ? store.getRow("organizations", orgId)
            : {};

        const name =
          typeof human.name === "string" && human.name.trim()
            ? human.name
            : null;
        const email =
          typeof human.email === "string" && human.email.trim()
            ? human.email
            : null;
        const jobTitle =
          typeof human.job_title === "string" && human.job_title.trim()
            ? human.job_title
            : null;
        const organizationName =
          typeof organization.name === "string" && organization.name.trim()
            ? organization.name
            : null;
        const memo =
          typeof human.memo === "string" && human.memo.trim()
            ? human.memo
            : null;

        if (!name && !email) return null;

        const details = [
          jobTitle,
          organizationName ? `Organization: ${organizationName}` : null,
          email ? `Email: ${email}` : null,
          memo ? `Notes: ${memo}` : null,
        ].filter(Boolean);

        return {
          kind: "text",
          text: [`Referenced contact: ${name ?? email}`, ...details].join("\n"),
        } satisfies ResolvedChatContext;
      }

      const organization = store.getRow("organizations", ref.organizationId);
      const name =
        typeof organization.name === "string" && organization.name.trim()
          ? organization.name
          : null;
      return name
        ? ({
            kind: "text",
            text: `Referenced organization: ${name}`,
          } satisfies ResolvedChatContext)
        : null;
    },
    [store],
  );
}

export function useEnhancerSessionIndex():
  | {
      transcriptIdsBySession: (sessionId: string) => string[];
      enhancedNoteIdsBySession: (sessionId: string) => string[];
    }
  | undefined {
  const indexes = useMainIndexes();

  const transcriptIdsBySession = useCallback(
    (sessionId: string) =>
      indexes ? listTranscriptIdsBySession(indexes, sessionId) : [],
    [indexes],
  );

  const enhancedNoteIdsBySession = useCallback(
    (sessionId: string) =>
      indexes ? listEnhancedNoteIdsBySession(indexes, sessionId) : [],
    [indexes],
  );

  return useMemo(() => {
    if (!indexes) return undefined;

    return { transcriptIdsBySession, enhancedNoteIdsBySession };
  }, [indexes, transcriptIdsBySession, enhancedNoteIdsBySession]);
}

// --- imperative helpers for non-hook callsites ----------------------------

export function listTranscriptIdsBySession(
  indexes: SliceIndexReader,
  sessionId: string,
): string[] {
  return indexes.getSliceRowIds(main.INDEXES.transcriptBySession, sessionId);
}

export function listEnhancedNoteIdsBySession(
  indexes: SliceIndexReader,
  sessionId: string,
): string[] {
  return indexes.getSliceRowIds(main.INDEXES.enhancedNotesBySession, sessionId);
}

type CapturedSessionData = NonNullable<ReturnType<typeof captureSessionData>>;

type AddDeletionFn = (
  capturedData: CapturedSessionData,
  onFinalize?: () => void,
  batchId?: string,
) => void;

type PendingDeletionMap = Record<
  string,
  {
    data: Parameters<typeof restoreSessionData>[1];
  }
>;

export function useDeleteSessionsWithUndo(): (args: {
  sessionIds: string[];
  invalidateSessionResource: (sessionId: string) => void;
  addDeletion: AddDeletionFn;
  batchId?: string;
}) => void {
  const store = useMainStore();
  const indexes = useMainIndexes();

  return useCallback(
    ({ sessionIds, invalidateSessionResource, addDeletion, batchId }) => {
      if (!store) return;

      for (const sessionId of sessionIds) {
        const capturedData = captureSessionData(store, indexes, sessionId);

        invalidateSessionResource(sessionId);
        void deleteSessionCascade(store, indexes, sessionId, {
          skipAudio: true,
        });

        if (!capturedData) continue;

        addDeletion(
          capturedData,
          () => {
            void fsSyncCommands.audioDelete(sessionId);
          },
          batchId,
        );
      }
    },
    [store, indexes],
  );
}

export function useRestoreDeletedSessions(): (args: {
  sessionIds: string[];
  pendingDeletions: PendingDeletionMap;
  clearDeletion: (sessionId: string) => void;
  clearBatch: (batchId: string) => void;
  openSession: (sessionId: string) => void;
  invalidateAudioQueries?: (sessionId: string) => void;
  batchKey?: string;
}) => void {
  const store = useMainStore();

  return useCallback(
    ({
      sessionIds,
      pendingDeletions,
      clearDeletion,
      clearBatch,
      openSession,
      invalidateAudioQueries,
      batchKey,
    }) => {
      if (!store) return;

      for (const sessionId of sessionIds) {
        const pending = pendingDeletions[sessionId];
        if (!pending) continue;
        restoreSessionData(
          store as Parameters<typeof restoreSessionData>[0],
          pending.data,
        );
        invalidateAudioQueries?.(sessionId);
      }

      if (sessionIds.length > 0) {
        openSession(sessionIds[0]);
      }

      if (batchKey) {
        clearBatch(batchKey);
      } else if (sessionIds[0]) {
        clearDeletion(sessionIds[0]);
      }
    },
    [store],
  );
}

export function useCreateSessionActions(): {
  createSession: () => string | null;
  getOrCreateSessionForEvent: (
    eventId: string,
    title?: string,
  ) => string | null;
} {
  const store = useMainStore();

  const createSessionAction = useCallback(() => {
    if (!store) return null;
    return createSession(store);
  }, [store]);

  const getOrCreateSessionForEvent = useCallback(
    (eventId: string, title?: string) => {
      if (!store) return null;
      return getOrCreateSessionForEventId(store, eventId, title);
    },
    [store],
  );

  return {
    createSession: createSessionAction,
    getOrCreateSessionForEvent,
  };
}

export function useCreateCountdownTestSession(): (
  seconds: number,
  meetingLink?: string,
) => string | null {
  const store = useMainStore();
  const userId = useCurrentUserId();

  return useCallback(
    (seconds: number, meetingLink?: string) => {
      if (!store) return null;
      const sessionId = crypto.randomUUID();
      const started_at = new Date(Date.now() + seconds * 1000).toISOString();
      const event_json = JSON.stringify({
        tracking_id: "devtool-test",
        calendar_id: "devtool-test",
        title: "Test Meeting",
        started_at,
        ended_at: new Date(
          Date.now() + seconds * 1000 + 30 * 60 * 1000,
        ).toISOString(),
        is_all_day: false,
        has_recurrence_rules: false,
        ...(meetingLink && { meeting_link: meetingLink }),
      });

      store.setRow("sessions", sessionId, {
        user_id: userId ?? "",
        created_at: new Date().toISOString(),
        title: meetingLink ? "Countdown Test (Zoom)" : "Countdown Test",
        event_json,
      });

      return sessionId;
    },
    [store, userId],
  );
}

export function useNearbyEventsForMicPrompt(): () => {
  id: string;
  title: string;
}[] {
  const eventsTable = main.UI.useTable("events", main.STORE_ID);

  return useCallback(() => {
    const now = Date.now();
    const windowMs = 15 * 60 * 1000;
    const results: { id: string; title: string; startedAt: number }[] = [];

    for (const [eventId, event] of Object.entries(eventsTable)) {
      if (!event?.started_at || event.is_all_day) continue;

      const startTime = new Date(String(event.started_at)).getTime();
      if (Number.isNaN(startTime)) continue;

      if (Math.abs(startTime - now) <= windowMs) {
        results.push({
          id: eventId,
          title: String(event.title || "Untitled Event"),
          startedAt: startTime,
        });
      }
    }

    results.sort((a, b) => a.startedAt - b.startedAt);
    return results.map(({ id, title }) => ({ id, title }));
  }, [eventsTable]);
}

export function useApplyImportedData(): (data: unknown) => Promise<void> {
  const store = useMainStore();

  return useCallback(
    async (data: unknown) => {
      if (!store) {
        throw new Error("Store not available");
      }
      const result = await importData(store as main.Store, data, save);
      if (result.status === "error") {
        throw new Error(result.error);
      }
    },
    [store],
  );
}
