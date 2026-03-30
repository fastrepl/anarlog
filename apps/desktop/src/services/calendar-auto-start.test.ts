import { describe, expect, test, vi } from "vitest";

import {
  consumePendingCalendarAutoStarts,
  resolveCalendarAutoStartEvent,
} from "./calendar-auto-start";

type MockStoreData = {
  events: Record<string, Record<string, unknown>>;
  values: Record<string, unknown>;
};

function createStore(data: MockStoreData) {
  return {
    forEachRow: (
      tableId: "events",
      callback: (rowId: string, forEachCell: unknown) => void,
    ) => {
      if (tableId !== "events") {
        return;
      }

      for (const rowId of Object.keys(data.events)) {
        callback(rowId, () => {});
      }
    },
    getRow: (tableId: "events", rowId: string) => {
      if (tableId !== "events") {
        return null;
      }

      return data.events[rowId] ?? null;
    },
    getValue: (valueId: string) => data.values[valueId],
  };
}

describe("resolveCalendarAutoStartEvent", () => {
  test("returns pending when the synced event row is not available yet", () => {
    const result = resolveCalendarAutoStartEvent(
      createStore({
        events: {},
        values: {},
      }),
      "tracking-1",
    );

    expect(result).toEqual({ status: "pending" });
  });

  test("returns ignored when the event tracking id is ignored", () => {
    const result = resolveCalendarAutoStartEvent(
      createStore({
        events: {
          "event-row-1": { tracking_id_event: "tracking-1" },
        },
        values: {
          ignored_events: JSON.stringify([{ tracking_id: "tracking-1" }]),
        },
      }),
      "tracking-1",
    );

    expect(result).toEqual({ status: "ignored" });
  });

  test("returns ignored when the recurring series is ignored", () => {
    const result = resolveCalendarAutoStartEvent(
      createStore({
        events: {
          "event-row-1": {
            tracking_id_event: "tracking-1",
            recurrence_series_id: "series-1",
          },
        },
        values: {
          ignored_recurring_series: JSON.stringify([{ id: "series-1" }]),
        },
      }),
      "tracking-1",
    );

    expect(result).toEqual({ status: "ignored" });
  });

  test("returns the local event row id when the event is ready", () => {
    const result = resolveCalendarAutoStartEvent(
      createStore({
        events: {
          "event-row-1": { tracking_id_event: "tracking-1" },
        },
        values: {},
      }),
      "tracking-1",
    );

    expect(result).toEqual({
      status: "ready",
      eventRowId: "event-row-1",
    });
  });
});

describe("consumePendingCalendarAutoStarts", () => {
  test("keeps unresolved tracking ids queued until sync catches up", () => {
    const pending = new Set(["tracking-1"]);
    const onReady = vi.fn();

    consumePendingCalendarAutoStarts(
      createStore({
        events: {},
        values: {},
      }),
      pending,
      onReady,
    );

    expect(onReady).not.toHaveBeenCalled();
    expect([...pending]).toEqual(["tracking-1"]);
  });

  test("consumes ready and ignored tracking ids without dropping future retries", () => {
    const pending = new Set(["tracking-ready", "tracking-ignored"]);
    const onReady = vi.fn();

    consumePendingCalendarAutoStarts(
      createStore({
        events: {
          "event-row-1": { tracking_id_event: "tracking-ready" },
          "event-row-2": { tracking_id_event: "tracking-ignored" },
        },
        values: {
          ignored_events: JSON.stringify([{ tracking_id: "tracking-ignored" }]),
        },
      }),
      pending,
      onReady,
    );

    expect(onReady).toHaveBeenCalledWith("event-row-1");
    expect([...pending]).toEqual([]);
  });
});
