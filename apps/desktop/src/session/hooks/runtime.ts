import { useCallback, useMemo } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import type { ContextEntity, ContextRef } from "~/chat/context/entities";
import { hydrateSessionContextFromFs } from "~/chat/context/session-context-hydrator";
import type { ResolvedChatContext } from "~/chat/transport";
import {
  type SliceIndexReader,
  useCurrentUserId,
  useMainIndexesInternal,
  useMainStoreInternal,
} from "~/session/hooks/internal";
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
  const store = useMainStoreInternal();

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
  const indexes = useMainIndexesInternal();

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

export function useDeleteSessionsWithUndo(): (args: {
  sessionIds: string[];
  invalidateSessionResource: (sessionId: string) => void;
  addDeletion: AddDeletionFn;
  batchId?: string;
}) => void {
  const store = useMainStoreInternal();
  const indexes = useMainIndexesInternal();

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
  const store = useMainStoreInternal();

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

export function useCreateSession(): () => string | null {
  const store = useMainStoreInternal();

  return useCallback(() => {
    if (!store) return null;
    return createSession(store);
  }, [store]);
}

export function useGetOrCreateSessionForEvent(): (
  eventId: string,
  title?: string,
) => string | null {
  const store = useMainStoreInternal();

  return useCallback(
    (eventId: string, title?: string) => {
      if (!store) return null;
      return getOrCreateSessionForEventId(store, eventId, title);
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
  const createSession = useCreateSession();
  const getOrCreateSessionForEvent = useGetOrCreateSessionForEvent();

  return useMemo(
    () => ({
      createSession,
      getOrCreateSessionForEvent,
    }),
    [createSession, getOrCreateSessionForEvent],
  );
}

export function useCreateCountdownTestSession(): (
  seconds: number,
  meetingLink?: string,
) => string | null {
  const store = useMainStoreInternal();
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
  const store = useMainStoreInternal();

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
