import { Node as PMNode } from "prosemirror-model";
import type { EditorView } from "prosemirror-view";
import { useCallback, useMemo } from "react";

import { parseJsonContent } from "~/editor/markdown";
import { type JSONContent, schema } from "~/editor/session";
import {
  getNodeTextContent,
  mergeLinkedSessionsIntoContent,
} from "~/editor/session/linked-session-content";
import {
  extractTasksFromContent,
  hydrateTaskContent,
  moveOpenTasksBetweenContents,
  normalizeTaskContent,
  type TaskSource,
} from "~/editor/tasks";
import {
  type MainStore,
  useMainStore,
  useUpdateDailyNoteContent,
} from "~/session/hooks/storage";
import {
  findSessionByEventId,
  findSessionByTrackingId,
  getSessionEventById,
} from "~/session/utils";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";
import type { TaskStorage } from "~/tasks/hooks";

type Store = MainStore;
const emptyDoc: JSONContent = { type: "doc", content: [{ type: "paragraph" }] };

function getSessionTitle(store: Store, sessionId: string): string {
  const title = store.getCell("sessions", sessionId, "title");
  return typeof title === "string" ? title : "";
}

function resolveEventSessionId(
  store: Store,
  eventId: string,
  createMissing = false,
): string | null {
  const existingSessionId = findSessionByEventId(store, eventId);
  if (existingSessionId) {
    return existingSessionId;
  }

  const event = store.getRow("events", eventId);
  if (!event) {
    return null;
  }

  return createMissing
    ? getOrCreateSessionForEventId(store, eventId, event.title as string)
    : null;
}

function normalizeSessionId(store: Store, sessionId: string): string {
  const trackingId = getSessionEventById(store, sessionId)?.tracking_id;
  if (!trackingId) {
    return sessionId;
  }

  return findSessionByTrackingId(store, trackingId) ?? sessionId;
}

function buildLinkedSessionIds(
  store: Store,
  eventIds: string[],
  sessionIds: string[],
): string[] {
  const linkedSessionIds: string[] = [];
  const seenSessionIds = new Set<string>();

  const pushSessionId = (sessionId: string | null) => {
    if (!sessionId) {
      return;
    }

    const normalizedSessionId = normalizeSessionId(store, sessionId);
    if (!normalizedSessionId || seenSessionIds.has(normalizedSessionId)) {
      return;
    }

    seenSessionIds.add(normalizedSessionId);
    linkedSessionIds.push(normalizedSessionId);
  };

  for (const eventId of eventIds) {
    pushSessionId(resolveEventSessionId(store, eventId));
  }

  for (const sessionId of sessionIds) {
    pushSessionId(sessionId);
  }

  return linkedSessionIds;
}

function buildLinkedContent(
  store: Store,
  content: JSONContent,
  eventIds: string[],
  sessionIds: string[],
): JSONContent {
  const linkedSessionIds = buildLinkedSessionIds(store, eventIds, sessionIds);
  const linkedSessionIdSet = new Set(linkedSessionIds);

  return mergeLinkedSessionsIntoContent({
    content,
    eventIds,
    sessionIds,
    resolveEventSessionId: (eventId) => resolveEventSessionId(store, eventId),
    getSessionTitle: (sessionId) => getSessionTitle(store, sessionId),
    normalizeSessionId: (sessionId) => normalizeSessionId(store, sessionId),
    keepLinkedSession: (sessionId) => linkedSessionIdSet.has(sessionId),
  });
}

function readRawContent(store: Store, date: string): JSONContent {
  const cell = store.getCell("daily_notes", date, "content");
  return normalizeTaskContent(parseJsonContent(cell as string)) ?? emptyDoc;
}

function syncLinkedSessions(
  view: EditorView,
  store: Store,
  eventIds: string[],
  sessionIds: string[],
): boolean {
  for (const eventId of eventIds) {
    resolveEventSessionId(store, eventId, true);
  }

  const currentContent = view.state.doc.toJSON() as JSONContent;
  const nextContent = buildLinkedContent(
    store,
    currentContent,
    eventIds,
    sessionIds,
  );
  if (JSON.stringify(nextContent) === JSON.stringify(currentContent)) {
    return false;
  }

  const nextDoc = PMNode.fromJSON(schema, nextContent);
  if (nextDoc.eq(view.state.doc)) {
    return false;
  }

  view.dispatch(
    view.state.tr.replaceWith(0, view.state.doc.content.size, nextDoc.content),
  );
  return true;
}

export function useDailyNoteEditorRuntime(date: string): {
  initializeContent: (args: {
    isToday?: boolean;
    previousDate: string;
    eventIds: string[];
    sessionIds: string[];
    taskStorage: TaskStorage | null;
    taskSource: TaskSource;
    previousTaskSource: TaskSource;
  }) => JSONContent | null;
  syncLinkedSessionsInView: (
    view: EditorView,
    eventIds: string[],
    sessionIds: string[],
  ) => void;
  syncSessionNodeTitles: (input: JSONContent) => void;
  persistDailyNote: (input: JSONContent) => void;
} {
  const store = useMainStore();
  const updateDailyNoteContent = useUpdateDailyNoteContent(date);

  const initializeContent = useCallback(
    ({
      isToday,
      previousDate,
      eventIds,
      sessionIds,
      taskStorage,
      taskSource,
      previousTaskSource,
    }: {
      isToday?: boolean;
      previousDate: string;
      eventIds: string[];
      sessionIds: string[];
      taskStorage: TaskStorage | null;
      taskSource: TaskSource;
      previousTaskSource: TaskSource;
    }) => {
      if (!store) return null;

      for (const eventId of eventIds) {
        resolveEventSessionId(store, eventId, true);
      }

      const rawContent = readRawContent(store, date);
      const linked = buildLinkedContent(
        store,
        rawContent,
        eventIds,
        sessionIds,
      );

      let content = linked;
      if (isToday && taskStorage) {
        const rawPrevious = readRawContent(store, previousDate);
        const currentCanonicalTasks = taskStorage.getTasksForSource(taskSource);
        const previousCanonicalTasks =
          taskStorage.getTasksForSource(previousTaskSource);

        const hydratedCurrent =
          currentCanonicalTasks.length > 0
            ? hydrateTaskContent({
                content: linked,
                sourceTasks: currentCanonicalTasks,
                getTask: taskStorage.getTask,
              })
            : linked;
        const hydratedPrevious =
          previousCanonicalTasks.length > 0
            ? hydrateTaskContent({
                content: rawPrevious,
                sourceTasks: previousCanonicalTasks,
                getTask: taskStorage.getTask,
              })
            : rawPrevious;

        const currentTasks =
          currentCanonicalTasks.length > 0
            ? currentCanonicalTasks
            : extractTasksFromContent(hydratedCurrent, taskSource);
        const previousTasks =
          previousCanonicalTasks.length > 0
            ? previousCanonicalTasks
            : extractTasksFromContent(hydratedPrevious, previousTaskSource);

        const carryForward = moveOpenTasksBetweenContents({
          previousContent: hydratedPrevious,
          currentContent: hydratedCurrent,
          previousTasks,
          currentTasks,
          currentSource: taskSource,
        });

        if (carryForward) {
          content = carryForward.currentContent;
          taskStorage.upsertTasksForSource(
            taskSource,
            carryForward.currentTasks,
          );
          taskStorage.upsertTasksForSource(
            previousTaskSource,
            carryForward.previousTasks,
          );

          if (
            JSON.stringify(carryForward.previousContent) !==
            JSON.stringify(rawPrevious)
          ) {
            store.setPartialRow("daily_notes", previousDate, {
              date: previousDate,
              content: JSON.stringify(carryForward.previousContent),
            });
          }
        } else {
          taskStorage.upsertTasksForSource(taskSource, currentTasks);
          taskStorage.upsertTasksForSource(previousTaskSource, previousTasks);
        }
      }

      if (JSON.stringify(content) !== JSON.stringify(rawContent)) {
        store.setPartialRow("daily_notes", date, {
          date,
          content: JSON.stringify(content),
        });
      }

      return content;
    },
    [date, store],
  );

  const syncLinkedSessionsInView = useCallback(
    (view: EditorView, eventIds: string[], sessionIds: string[]) => {
      if (!store) return;
      syncLinkedSessions(view, store, eventIds, sessionIds);
    },
    [store],
  );

  const syncSessionNodeTitles = useCallback(
    (input: JSONContent) => {
      if (!store) return;

      for (const node of input.content ?? []) {
        if (node.type !== "session") {
          continue;
        }

        const sessionId = node.attrs?.sessionId;
        if (typeof sessionId !== "string" || sessionId === "") {
          continue;
        }

        const nextTitle = getNodeTextContent(node);
        const currentTitle = getSessionTitle(store, sessionId);
        if (nextTitle !== currentTitle) {
          store.setPartialRow("sessions", sessionId, { title: nextTitle });
        }
      }
    },
    [store],
  );

  const persistDailyNote = useCallback(
    (input: JSONContent) => updateDailyNoteContent(JSON.stringify(input)),
    [updateDailyNoteContent],
  );

  return useMemo(
    () => ({
      initializeContent,
      syncLinkedSessionsInView,
      syncSessionNodeTitles,
      persistDailyNote,
    }),
    [
      initializeContent,
      syncLinkedSessionsInView,
      syncSessionNodeTitles,
      persistDailyNote,
    ],
  );
}
