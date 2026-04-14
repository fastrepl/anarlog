import "./note-editor.css";

import { Node as PMNode } from "prosemirror-model";
import type { EditorView } from "prosemirror-view";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { parseJsonContent } from "@hypr/tiptap/shared";
import { format, parseISO, subDays } from "@hypr/utils";

import { useCalendarData } from "~/calendar/hooks";
import { getEventById } from "~/calendar/queries";
import {
  type JSONContent,
  NoteEditor,
  type NoteEditorRef,
  schema,
} from "~/editor/session";
import {
  getNodeTextContent,
  mergeLinkedSessionsIntoContent,
} from "~/editor/session/linked-session-content";
import { useTaskStorageOptional } from "~/editor/task-storage";
import {
  extractTasksFromContent,
  hydrateTaskContent,
  moveOpenTasksBetweenContents,
  normalizeTaskContent,
} from "~/editor/tasks";
import {
  findSessionByEventId,
  findSessionByTrackingId,
  getSessionEventById,
} from "~/session/utils";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;
const emptyDoc: JSONContent = { type: "doc", content: [{ type: "paragraph" }] };

function getSessionTitle(store: Store, sessionId: string): string {
  const title = store.getCell("sessions", sessionId, "title");
  return typeof title === "string" ? title : "";
}

async function resolveEventSessionId(
  store: Store,
  eventId: string,
  createMissing = false,
): Promise<string | null> {
  const existingSessionId = await findSessionByEventId(store, eventId);
  if (existingSessionId) {
    return existingSessionId;
  }

  const event = await getEventById(eventId);
  if (!event) {
    return null;
  }

  return createMissing
    ? getOrCreateSessionForEventId(store, eventId, event.title)
    : null;
}

function normalizeSessionId(store: Store, sessionId: string): string {
  const trackingId = getSessionEventById(store, sessionId)?.tracking_id;
  if (!trackingId) {
    return sessionId;
  }

  return findSessionByTrackingId(store, trackingId) ?? sessionId;
}

async function buildEventSessionMap(
  store: Store,
  eventIds: string[],
): Promise<Map<string, string | null>> {
  const map = new Map<string, string | null>();
  for (const eventId of eventIds) {
    map.set(eventId, await resolveEventSessionId(store, eventId));
  }
  return map;
}

function buildLinkedSessionIds(
  store: Store,
  eventSessionMap: Map<string, string | null>,
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

  for (const [, sessionId] of eventSessionMap) {
    pushSessionId(sessionId);
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
  eventSessionMap: Map<string, string | null>,
): JSONContent {
  const linkedSessionIds = buildLinkedSessionIds(
    store,
    eventSessionMap,
    sessionIds,
  );
  const linkedSessionIdSet = new Set(linkedSessionIds);

  return mergeLinkedSessionsIntoContent({
    content,
    eventIds,
    sessionIds,
    resolveEventSessionId: (eventId) => eventSessionMap.get(eventId) ?? null,
    getSessionTitle: (sessionId) => getSessionTitle(store, sessionId),
    normalizeSessionId: (sessionId) => normalizeSessionId(store, sessionId),
    keepLinkedSession: (sessionId) => linkedSessionIdSet.has(sessionId),
  });
}

async function syncLinkedSessions(
  view: EditorView,
  store: Store,
  eventIds: string[],
  sessionIds: string[],
): Promise<boolean> {
  for (const eventId of eventIds) {
    await resolveEventSessionId(store, eventId, true);
  }

  const eventSessionMap = await buildEventSessionMap(store, eventIds);
  const currentContent = view.state.doc.toJSON() as JSONContent;
  const nextContent = buildLinkedContent(
    store,
    currentContent,
    eventIds,
    sessionIds,
    eventSessionMap,
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

function readRawContent(store: Store, date: string): JSONContent {
  const cell = store.getCell("daily_notes", date, "content");
  return normalizeTaskContent(parseJsonContent(cell as string)) ?? emptyDoc;
}

function isEditorTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement && target.closest(".ProseMirror") !== null
  );
}

export function DailyNoteEditor({
  date,
  isToday,
}: {
  date: string;
  isToday?: boolean;
}) {
  const store = main.UI.useStore(main.STORE_ID);
  const editorRef = useRef<NoteEditorRef>(null);
  const taskStorage = useTaskStorageOptional();
  const taskSource = useMemo(() => ({ type: "daily_note", id: date }), [date]);
  const previousDate = useMemo(
    () => format(subDays(parseISO(`${date}T00:00:00`), 1), "yyyy-MM-dd"),
    [date],
  );
  const previousTaskSource = useMemo(
    () => ({ type: "daily_note", id: previousDate }),
    [previousDate],
  );

  const { eventIdsByDate, sessionIdsByDate } = useCalendarData();
  const eventIds = eventIdsByDate[date] ?? [];
  const sessionIds = sessionIdsByDate[date] ?? [];

  // Compute initial content once on mount via async init.
  // This breaks the read→derive→write→read loop that `useCell` would create.
  const [initialContent, setInitialContent] = useState<JSONContent | null>(
    null,
  );
  const initStartedRef = useRef(false);
  useEffect(() => {
    if (initStartedRef.current || !store) return;
    initStartedRef.current = true;

    (async () => {
      for (const eventId of eventIds) {
        await resolveEventSessionId(store, eventId, true);
      }

      const eventSessionMap = await buildEventSessionMap(store, eventIds);
      const rawContent = readRawContent(store, date);
      const linked = buildLinkedContent(
        store,
        rawContent,
        eventIds,
        sessionIds,
        eventSessionMap,
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

      setInitialContent(content);
    })();
  }, [store]);

  const persistDailyNote = main.UI.useSetPartialRowCallback(
    "daily_notes",
    date,
    (input: JSONContent) => ({ content: JSON.stringify(input), date }),
    [date],
    main.STORE_ID,
  );

  useEffect(() => {
    const view = editorRef.current?.view;
    if (!store || !view) {
      return;
    }

    void syncLinkedSessions(view, store, eventIds, sessionIds).catch(() => {
      // invalid content
    });
  }, [eventIds, sessionIds, store]);

  const handleChange = useCallback(
    (input: JSONContent) => {
      if (store) {
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
      }

      persistDailyNote(input);
    },
    [persistDailyNote, store],
  );

  const focusEditor = useCallback(() => {
    editorRef.current?.commands.focus();
  }, []);

  const handleContainerMouseDownCapture = useCallback(
    (event: React.MouseEvent) => {
      if (isEditorTarget(event.target)) {
        return;
      }

      event.preventDefault();
      focusEditor();
    },
    [focusEditor],
  );

  const handleContainerClick = useCallback(
    (event: React.MouseEvent) => {
      if (isEditorTarget(event.target)) {
        return;
      }

      focusEditor();
    },
    [focusEditor],
  );

  if (!initialContent) {
    return null;
  }

  return (
    <div
      className="main2-daily-note-editor flex-1 px-6"
      onMouseDownCapture={handleContainerMouseDownCapture}
      onClick={handleContainerClick}
    >
      <NoteEditor
        ref={editorRef}
        key={`daily-${date}`}
        initialContent={initialContent}
        handleChange={handleChange}
        linkedItemOpenBehavior="new"
        taskSource={taskSource}
      />
    </div>
  );
}
