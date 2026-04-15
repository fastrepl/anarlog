import "./note-editor.css";

import { Node as PMNode } from "prosemirror-model";
import type { EditorView } from "prosemirror-view";
import { useCallback, useEffect, useMemo, useRef } from "react";

import { parseJsonContent } from "@hypr/tiptap/shared";
import { format, parseISO, subDays } from "@hypr/utils";

import { welcomeContent } from "./welcome-content";

import { useCalendarData } from "~/calendar/hooks";
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
import { useTabs } from "~/store/zustand/tabs";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;
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

function readRawContent(store: Store, date: string): JSONContent {
  const cell = store.getCell("daily_notes", date, "content");
  return normalizeTaskContent(parseJsonContent(cell as string)) ?? emptyDoc;
}

function hasWelcomeContent(content: JSONContent): boolean {
  const firstWelcomeNode = welcomeContent.content?.[0];
  const firstContentNode = content.content?.[0];
  if (!firstWelcomeNode || !firstContentNode) return false;
  return JSON.stringify(firstContentNode) === JSON.stringify(firstWelcomeNode);
}

const WELCOME_NODE_COUNT = (welcomeContent.content?.length ?? 0) + 1; // +1 for separator paragraph

function prependWelcome(content: JSONContent): JSONContent {
  const welcomeNodes = welcomeContent.content ?? [];
  const existingNodes = content.content ?? [];
  return {
    type: "doc",
    content: [...welcomeNodes, { type: "paragraph" }, ...existingNodes],
  };
}

function stripWelcome(content: JSONContent): JSONContent {
  let result = content;
  while (hasWelcomeContent(result)) {
    const nodes = result.content ?? [];
    const remaining = nodes.slice(WELCOME_NODE_COUNT);
    result = {
      type: "doc",
      content: remaining.length > 0 ? remaining : [{ type: "paragraph" }],
    };
  }
  return result;
}

export function DailyNoteEditor({
  date,
  isToday,
  showWelcome,
}: {
  date: string;
  isToday?: boolean;
  showWelcome?: boolean;
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

  // Compute initial content once on mount — imperative read, no subscription.
  // This breaks the read→derive→write→read loop that `useCell` would create.
  const initialContentRef = useRef<JSONContent | null>(null);
  if (!initialContentRef.current && store) {
    for (const eventId of eventIds) {
      resolveEventSessionId(store, eventId, true);
    }

    const rawContent = readRawContent(store, date);
    const linked = buildLinkedContent(store, rawContent, eventIds, sessionIds);

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
        taskStorage.upsertTasksForSource(taskSource, carryForward.currentTasks);
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

    // Strip any previously persisted welcome content
    const cleanedContent = stripWelcome(content);
    if (JSON.stringify(cleanedContent) !== JSON.stringify(rawContent)) {
      content = cleanedContent;
      store.setPartialRow("daily_notes", date, {
        date,
        content: JSON.stringify(cleanedContent),
      });
    }

    if (showWelcome) {
      content = prependWelcome(stripWelcome(content));
    }

    initialContentRef.current = content;
  }

  const persistDailyNote = main.UI.useSetPartialRowCallback(
    "daily_notes",
    date,
    (input: JSONContent) => ({ content: JSON.stringify(input), date }),
    [date],
    main.STORE_ID,
  );

  useEffect(() => {
    if (!showWelcome) {
      const view = editorRef.current?.view;
      if (view && hasWelcomeContent(view.state.doc.toJSON() as JSONContent)) {
        const currentContent = view.state.doc.toJSON() as JSONContent;
        const cleaned = stripWelcome(currentContent);
        const nextDoc = PMNode.fromJSON(schema, cleaned);
        view.dispatch(
          view.state.tr.replaceWith(
            0,
            view.state.doc.content.size,
            nextDoc.content,
          ),
        );
      }
    }
  }, [showWelcome]);

  useEffect(() => {
    const view = editorRef.current?.view;
    if (!store || !view) {
      return;
    }

    try {
      syncLinkedSessions(view, store, eventIds, sessionIds);
    } catch {
      // invalid content
    }
  }, [eventIds, sessionIds, store]);

  const handleChange = useCallback(
    (input: JSONContent) => {
      const contentToSave = showWelcome ? stripWelcome(input) : input;

      if (store) {
        for (const node of contentToSave.content ?? []) {
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

      persistDailyNote(contentToSave);
    },
    [persistDailyNote, store, showWelcome],
  );

  const openNew = useTabs((s) => s.openNew);
  const handleLinkClick = useCallback(
    (e: React.MouseEvent) => {
      const anchor = (e.target as HTMLElement).closest("a[href]");
      if (!anchor) return;
      const href = anchor.getAttribute("href");
      if (!href) return;

      if (href.startsWith("char://settings/")) {
        e.preventDefault();
        const tab = href.replace("char://settings/", "");
        openNew({ type: "settings", state: { tab } });
      }
    },
    [openNew],
  );

  if (!initialContentRef.current) {
    return null;
  }

  return (
    <div className="main2-daily-note-editor px-6" onClick={handleLinkClick}>
      <NoteEditor
        ref={editorRef}
        key={`daily-${date}`}
        initialContent={initialContentRef.current}
        handleChange={handleChange}
        linkedItemOpenBehavior="new"
        taskSource={taskSource}
      />
    </div>
  );
}
