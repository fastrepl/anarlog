import { useCallback, useMemo, useRef, useSyncExternalStore } from "react";

import type {
  IgnoredEvent,
  IgnoredRecurringSeries,
  SessionEvent,
} from "@hypr/store";

import { getSessionEvent } from "~/session/utils";
import * as main from "~/store/tinybase/store/main";

export function useSessionEvent(sessionId: string): SessionEvent | null {
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  return useMemo(() => getSessionEvent({ event_json: eventJson }), [eventJson]);
}

export function useEvent(eventId: string | undefined) {
  const title = main.UI.useCell(
    "events",
    eventId ?? "",
    "title",
    main.STORE_ID,
  );
  const startedAt = main.UI.useCell(
    "events",
    eventId ?? "",
    "started_at",
    main.STORE_ID,
  );
  const endedAt = main.UI.useCell(
    "events",
    eventId ?? "",
    "ended_at",
    main.STORE_ID,
  );
  const location = main.UI.useCell(
    "events",
    eventId ?? "",
    "location",
    main.STORE_ID,
  );
  const meetingLink = main.UI.useCell(
    "events",
    eventId ?? "",
    "meeting_link",
    main.STORE_ID,
  );
  const description = main.UI.useCell(
    "events",
    eventId ?? "",
    "description",
    main.STORE_ID,
  );
  const calendarId = main.UI.useCell(
    "events",
    eventId ?? "",
    "calendar_id",
    main.STORE_ID,
  );

  return useMemo(
    () =>
      eventId
        ? {
            title,
            startedAt,
            endedAt,
            location,
            meetingLink,
            description,
            calendarId,
          }
        : null,
    [
      eventId,
      title,
      startedAt,
      endedAt,
      location,
      meetingLink,
      description,
      calendarId,
    ],
  );
}

function parseIgnoredEvents(raw: string | undefined): IgnoredEvent[] {
  if (!raw) return [];
  try {
    return JSON.parse(raw) as IgnoredEvent[];
  } catch {
    return [];
  }
}

function parseIgnoredSeries(raw: string | undefined): IgnoredRecurringSeries[] {
  if (!raw) return [];
  try {
    return JSON.parse(raw) as IgnoredRecurringSeries[];
  } catch {
    return [];
  }
}

export function useIgnoredEvents() {
  const store = main.UI.useStore(main.STORE_ID);

  const ignoredEventsRaw = main.UI.useValue("ignored_events", main.STORE_ID) as
    | string
    | undefined;
  const ignoredSeriesRaw = main.UI.useValue(
    "ignored_recurring_series",
    main.STORE_ID,
  ) as string | undefined;

  const ignoredIds = useMemo(() => {
    const list = parseIgnoredEvents(ignoredEventsRaw);
    return new Set(list.map((e) => e.tracking_id));
  }, [ignoredEventsRaw]);

  const ignoredSeriesIds = useMemo(() => {
    const list = parseIgnoredSeries(ignoredSeriesRaw);
    return new Set(list.map((e) => e.id));
  }, [ignoredSeriesRaw]);

  const isIgnored = useCallback(
    (
      trackingId: string | null | undefined,
      recurrenceSeriesId: string | null | undefined,
    ) => {
      if (!trackingId) return false;
      if (ignoredIds.has(trackingId)) return true;
      if (recurrenceSeriesId && ignoredSeriesIds.has(recurrenceSeriesId))
        return true;
      return false;
    },
    [ignoredIds, ignoredSeriesIds],
  );

  const ignoreEvent = useCallback(
    (trackingId: string) => {
      if (!store) return;
      const list = parseIgnoredEvents(
        store.getValue("ignored_events") as string | undefined,
      );
      const now = new Date().toISOString();
      list.push({
        tracking_id: trackingId,
        last_seen: now,
      });
      store.setValue("ignored_events", JSON.stringify(list));
    },
    [store],
  );

  const unignoreEvent = useCallback(
    (trackingId: string) => {
      if (!store) return;
      const list = parseIgnoredEvents(
        store.getValue("ignored_events") as string | undefined,
      );
      const filtered = list.filter((e) => e.tracking_id !== trackingId);
      store.setValue("ignored_events", JSON.stringify(filtered));
    },
    [store],
  );

  const ignoreSeries = useCallback(
    (seriesId: string) => {
      if (!store) return;
      const list = parseIgnoredSeries(
        store.getValue("ignored_recurring_series") as string | undefined,
      );
      if (!list.some((e) => e.id === seriesId)) {
        list.push({ id: seriesId, last_seen: new Date().toISOString() });
        store.setValue("ignored_recurring_series", JSON.stringify(list));
      }
    },
    [store],
  );

  const unignoreSeries = useCallback(
    (seriesId: string) => {
      if (!store) return;
      const list = parseIgnoredSeries(
        store.getValue("ignored_recurring_series") as string | undefined,
      );
      store.setValue(
        "ignored_recurring_series",
        JSON.stringify(list.filter((e) => e.id !== seriesId)),
      );
    },
    [store],
  );

  return {
    isIgnored,
    ignoreEvent,
    unignoreEvent,
    ignoreSeries,
    unignoreSeries,
  };
}

type UiStore = NonNullable<ReturnType<typeof main.UI.useStore>>;
type StoreTableId = Parameters<UiStore["addRowListener"]>[0];

export function useMainStoreRowsRevision(
  tableId: StoreTableId,
  rowIds: readonly string[],
): number {
  const store = main.UI.useStore(main.STORE_ID);

  return useStoreRowsRevision(store, tableId, rowIds);
}

export function useStoreRowsRevision(
  store: UiStore | undefined,
  tableId: StoreTableId,
  rowIds: readonly string[],
): number {
  const revisionRef = useRef(0);
  const rowIdsKey = getRowIdsKey(rowIds);
  const subscribedRowIds = useMemo(() => getUniqueRowIds(rowIds), [rowIdsKey]);

  const subscribe = useCallback(
    (notify: () => void) => {
      if (!store || subscribedRowIds.length === 0) {
        return noop;
      }

      const listenerIds = subscribedRowIds.map((rowId) =>
        store.addRowListener(tableId, rowId, () => {
          revisionRef.current += 1;
          notify();
        }),
      );

      return () => {
        for (const listenerId of listenerIds) {
          store.delListener(listenerId);
        }
      };
    },
    [store, subscribedRowIds, tableId],
  );
  const getSnapshot = useCallback(() => revisionRef.current, []);

  return useSyncExternalStore(subscribe, getSnapshot, getZero);
}

function getRowIdsKey(rowIds: readonly string[]): string {
  return getUniqueRowIds(rowIds).join("\u0000");
}

export function getUniqueRowIds(rowIds: readonly string[]): string[] {
  const uniqueRowIds: string[] = [];
  const seen = new Set<string>();

  for (const rowId of rowIds) {
    if (!rowId || seen.has(rowId)) {
      continue;
    }

    uniqueRowIds.push(rowId);
    seen.add(rowId);
  }

  return uniqueRowIds;
}

function noop() {}

function getZero() {
  return 0;
}
