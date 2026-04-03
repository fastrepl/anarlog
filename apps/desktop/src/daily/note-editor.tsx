import { useCallback, useEffect, useMemo, useRef } from "react";

import { parseJsonContent } from "@hypr/tiptap/shared";

import { useCalendarData } from "~/calendar/hooks";
import { type JSONContent, NoteEditor } from "~/editor/session";
import {
  getNodeTextContent,
  mergeLinkedSessionsIntoContent,
} from "~/editor/session/linked-session-content";
import {
  findSessionByEventId,
  findSessionByTrackingId,
  getSessionEventById,
} from "~/session/utils";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

function hasLinkedSessionContent(content: JSONContent): boolean {
  return (content.content ?? []).some(
    (node) => node.type === "event" || node.type === "session",
  );
}

function getSessionTitle(store: Store, sessionId: string): string {
  const title = store.getCell("sessions", sessionId, "title");
  return typeof title === "string" ? title : "";
}

function resolveEventSessionId(store: Store, eventId: string): string | null {
  const existingSessionId = findSessionByEventId(store, eventId);
  if (existingSessionId) {
    return existingSessionId;
  }

  const event = store.getRow("events", eventId);
  if (!event) {
    return null;
  }

  return getOrCreateSessionForEventId(store, eventId, event.title as string);
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

function buildInitialContent(
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

export function DailyNoteEditor({ date }: { date: string }) {
  const store = main.UI.useStore(main.STORE_ID);
  const content = main.UI.useCell(
    "daily_notes",
    date,
    "content",
    main.STORE_ID,
  );

  const { eventIdsByDate, sessionIdsByDate } = useCalendarData();
  const rawContent = useMemo(
    () => parseJsonContent(content as string),
    [content],
  );
  const initialContent = useMemo(() => {
    if (!store) {
      return null;
    }

    return buildInitialContent(
      store,
      rawContent,
      eventIdsByDate[date] ?? [],
      sessionIdsByDate[date] ?? [],
    );
  }, [date, eventIdsByDate, rawContent, sessionIdsByDate, store]);
  const rawContentHasLinkedNodes = useMemo(
    () => hasLinkedSessionContent(rawContent),
    [rawContent],
  );
  const shouldPersistNormalizedContent = useMemo(() => {
    if (!initialContent || !rawContentHasLinkedNodes) {
      return false;
    }

    return JSON.stringify(initialContent) !== JSON.stringify(rawContent);
  }, [initialContent, rawContent, rawContentHasLinkedNodes]);
  const initialContentRef = useRef<JSONContent | null>(null);
  const initialContentKeyRef = useRef<string | null>(null);
  const initialContentKey = useMemo(
    () =>
      JSON.stringify({
        date,
        content,
        eventIds: eventIdsByDate[date] ?? [],
        sessionIds: sessionIdsByDate[date] ?? [],
      }),
    [content, date, eventIdsByDate, sessionIdsByDate],
  );

  if (initialContent && initialContentKeyRef.current !== initialContentKey) {
    initialContentRef.current = initialContent;
    initialContentKeyRef.current = initialContentKey;
  }

  const persistDailyNote = main.UI.useSetPartialRowCallback(
    "daily_notes",
    date,
    (input: JSONContent) => ({ content: JSON.stringify(input), date }),
    [date],
    main.STORE_ID,
  );

  useEffect(() => {
    if (!initialContent || !shouldPersistNormalizedContent) {
      return;
    }

    persistDailyNote(initialContent);
  }, [initialContent, persistDailyNote, shouldPersistNormalizedContent]);

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

  if (!initialContentRef.current) {
    return null;
  }

  return (
    <div className="px-2">
      <NoteEditor
        key={`daily-${date}`}
        initialContent={initialContentRef.current}
        handleChange={handleChange}
        linkedItemOpenBehavior="new"
      />
    </div>
  );
}
