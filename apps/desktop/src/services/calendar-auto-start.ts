type StoreLike = {
  forEachRow: (
    tableId: "events",
    callback: (rowId: string, forEachCell: unknown) => void,
  ) => void;
  getRow: (tableId: "events", rowId: string) => Record<string, unknown> | null;
  getValue: (valueId: "ignored_events" | "ignored_recurring_series") => unknown;
};

export type CalendarAutoStartResolution =
  | { status: "pending" }
  | { status: "ignored" }
  | { status: "ready"; eventRowId: string };

export function resolveCalendarAutoStartEvent(
  store: StoreLike,
  trackingId: string,
): CalendarAutoStartResolution {
  let eventRowId: string | null = null;

  store.forEachRow("events", (rowId, _forEachCell) => {
    if (eventRowId) return;
    const row = store.getRow("events", rowId);
    if (row?.tracking_id_event === trackingId) {
      eventRowId = rowId;
    }
  });

  if (!eventRowId) {
    return { status: "pending" };
  }

  const eventRow = store.getRow("events", eventRowId);
  const seriesId = eventRow?.recurrence_series_id as string | undefined;

  if (isTrackingIdIgnored(store, trackingId)) {
    return { status: "ignored" };
  }

  if (seriesId && isRecurringSeriesIgnored(store, seriesId)) {
    return { status: "ignored" };
  }

  return { status: "ready", eventRowId };
}

export function consumePendingCalendarAutoStarts(
  store: StoreLike,
  pendingTrackingIds: Set<string>,
  onReady: (eventRowId: string) => void,
): void {
  for (const trackingId of [...pendingTrackingIds]) {
    const resolution = resolveCalendarAutoStartEvent(store, trackingId);

    if (resolution.status === "pending") {
      continue;
    }

    pendingTrackingIds.delete(trackingId);

    if (resolution.status === "ready") {
      onReady(resolution.eventRowId);
    }
  }
}

function isTrackingIdIgnored(store: StoreLike, trackingId: string): boolean {
  try {
    const raw = store.getValue("ignored_events");
    if (!raw || typeof raw !== "string") {
      return false;
    }

    const ignored = JSON.parse(raw) as Array<{ tracking_id: string }>;
    return ignored.some((event) => event.tracking_id === trackingId);
  } catch {
    return false;
  }
}

function isRecurringSeriesIgnored(store: StoreLike, seriesId: string): boolean {
  try {
    const raw = store.getValue("ignored_recurring_series");
    if (!raw || typeof raw !== "string") {
      return false;
    }

    const ignored = JSON.parse(raw) as Array<{ id: string }>;
    return ignored.some((series) => series.id === seriesId);
  } catch {
    return false;
  }
}
