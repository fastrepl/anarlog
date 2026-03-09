import { describe, expect, test } from "vitest";

import type { SessionEvent } from "@hypr/store";

import type { Ctx } from "../../ctx";
import type { IncomingEvent } from "../../fetch/types";
import { syncSessionEmbeddedEvents } from "./execute";

type MockStoreData = {
  sessions: Record<string, Record<string, unknown>>;
  events: Record<string, Record<string, unknown>>;
  values: Record<string, string>;
};

function createMockStore(data: MockStoreData) {
  return {
    getRow: (table: string, id: string) => {
      if (table === "sessions") return data.sessions[id] ?? {};
      if (table === "events") return data.events[id] ?? {};
      return {};
    },
    forEachRow: (
      table: string,
      callback: (id: string, forEachCell: unknown) => void,
    ) => {
      const tableData =
        table === "sessions"
          ? data.sessions
          : table === "events"
            ? data.events
            : {};
      for (const id of Object.keys(tableData)) {
        callback(id, () => {});
      }
    },
    setPartialRow: (
      table: string,
      id: string,
      row: Record<string, unknown>,
    ) => {
      if (table === "sessions") {
        data.sessions[id] = { ...data.sessions[id], ...row };
      }
    },
    transaction: (fn: () => void) => fn(),
    getValue: (key: string) => data.values[key],
    setValue: (key: string, value: string) => {
      data.values[key] = value;
    },
  } as unknown as Ctx["store"];
}

function createMockCtx(
  storeData: MockStoreData,
  overrides: Partial<Ctx> = {},
): Ctx {
  return {
    store: createMockStore(storeData),
    provider: "apple" as const,
    userId: "user-1",
    from: new Date("2024-01-01"),
    to: new Date("2024-02-01"),
    calendarIds: new Set(["cal-1"]),
    calendarTrackingIdToId: new Map([["tracking-cal-1", "cal-1"]]),
    ...overrides,
  };
}

function makeSessionEvent(overrides: Partial<SessionEvent> = {}): SessionEvent {
  return {
    tracking_id: "track-1",
    calendar_id: "cal-1",
    title: "Old Title",
    started_at: "2024-01-15T10:00:00Z",
    ended_at: "2024-01-15T11:00:00Z",
    is_all_day: false,
    has_recurrence_rules: false,
    ...overrides,
  };
}

function makeIncomingEvent(
  overrides: Partial<IncomingEvent> = {},
): IncomingEvent {
  return {
    tracking_id_event: "track-1",
    tracking_id_calendar: "tracking-cal-1",
    title: "Updated Title",
    started_at: "2024-01-15T10:00:00Z",
    ended_at: "2024-01-15T11:00:00Z",
    has_recurrence_rules: false,
    is_all_day: false,
    ...overrides,
  };
}

describe("syncSessionEmbeddedEvents", () => {
  test("updates session embedded event for non-recurring event", () => {
    const storeData: MockStoreData = {
      sessions: {
        "session-1": {
          event_json: JSON.stringify(makeSessionEvent()),
        },
      },
      events: {},
      values: {},
    };
    const ctx = createMockCtx(storeData);

    syncSessionEmbeddedEvents(ctx, [
      makeIncomingEvent({ title: "Updated Title" }),
    ]);

    const updated = JSON.parse(
      storeData.sessions["session-1"].event_json as string,
    );
    expect(updated.title).toBe("Updated Title");
    expect(updated.tracking_id).toBe("track-1");
  });

  test("matches recurring events by unique tracking_id per occurrence", () => {
    const storeData: MockStoreData = {
      sessions: {
        "session-jan15": {
          event_json: JSON.stringify(
            makeSessionEvent({
              tracking_id: "recurring-1:2024-01-15",
              has_recurrence_rules: true,
              started_at: "2024-01-15T10:00:00Z",
            }),
          ),
        },
        "session-jan22": {
          event_json: JSON.stringify(
            makeSessionEvent({
              tracking_id: "recurring-1:2024-01-22",
              has_recurrence_rules: true,
              started_at: "2024-01-22T10:00:00Z",
            }),
          ),
        },
      },
      events: {},
      values: {},
    };
    const ctx = createMockCtx(storeData);

    syncSessionEmbeddedEvents(ctx, [
      makeIncomingEvent({
        tracking_id_event: "recurring-1:2024-01-15",
        has_recurrence_rules: true,
        started_at: "2024-01-15T10:00:00Z",
        title: "Updated Jan 15",
      }),
    ]);

    const jan15 = JSON.parse(
      storeData.sessions["session-jan15"].event_json as string,
    );
    expect(jan15.title).toBe("Updated Jan 15");

    const jan22 = JSON.parse(
      storeData.sessions["session-jan22"].event_json as string,
    );
    expect(jan22.title).toBe("Old Title");
  });

  test("skips sessions without embedded events", () => {
    const storeData: MockStoreData = {
      sessions: {
        "session-1": { title: "No Event" },
      },
      events: {},
      values: {},
    };
    const ctx = createMockCtx(storeData);

    syncSessionEmbeddedEvents(ctx, [makeIncomingEvent()]);

    expect(storeData.sessions["session-1"].event_json).toBeUndefined();
  });

  test("does nothing when incoming events is empty", () => {
    const original = makeSessionEvent();
    const storeData: MockStoreData = {
      sessions: {
        "session-1": {
          event_json: JSON.stringify(original),
        },
      },
      events: {},
      values: {},
    };
    const ctx = createMockCtx(storeData);

    syncSessionEmbeddedEvents(ctx, []);

    const result = JSON.parse(
      storeData.sessions["session-1"].event_json as string,
    );
    expect(result.title).toBe("Old Title");
  });

  test("resolves calendar_id from calendarTrackingIdToId map", () => {
    const storeData: MockStoreData = {
      sessions: {
        "session-1": {
          event_json: JSON.stringify(
            makeSessionEvent({ calendar_id: "old-cal" }),
          ),
        },
      },
      events: {},
      values: {},
    };
    const ctx = createMockCtx(storeData, {
      calendarTrackingIdToId: new Map([["tracking-cal-new", "cal-new"]]),
    });

    syncSessionEmbeddedEvents(ctx, [
      makeIncomingEvent({ tracking_id_calendar: "tracking-cal-new" }),
    ]);

    const result = JSON.parse(
      storeData.sessions["session-1"].event_json as string,
    );
    expect(result.calendar_id).toBe("cal-new");
  });
});
