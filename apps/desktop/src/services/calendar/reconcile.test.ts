import { createMergeableStore } from "tinybase/with-schemas";
import { describe, expect, test } from "vitest";

import { SCHEMA, type SessionEvent } from "@hypr/store";

import { reconcileCalendarSessions } from "./reconcile";

import type { Store } from "~/store/tinybase/store/main";

function createStore() {
  return createMergeableStore()
    .setTablesSchema(SCHEMA.table)
    .setValuesSchema(SCHEMA.value) as Store;
}

function makeSessionEvent(overrides: Partial<SessionEvent> = {}): SessionEvent {
  return {
    tracking_id: "event-1",
    calendar_id: "old-calendar-id",
    title: "Standup",
    started_at: "2026-04-15T09:00:00Z",
    ended_at: "2026-04-15T09:30:00Z",
    is_all_day: false,
    has_recurrence_rules: false,
    ...overrides,
  };
}

describe("reconcileCalendarSessions", () => {
  test("keeps session calendar ids tied to the source event row when a row id matches another tracking id", () => {
    const store = createStore();

    store.setValue("user_id", "user-1");

    store.setRow("calendars", "tracking-cal-2", {
      user_id: "user-1",
      created_at: "2026-04-15T00:00:00Z",
      tracking_id_calendar: "tracking-cal-1",
      name: "John (Char)",
      enabled: true,
      provider: "google",
      source: "john@char.com",
      color: "#4285f4",
      connection_id: "conn-john",
    });
    store.setRow("calendars", "gmail-row", {
      user_id: "user-1",
      created_at: "2026-04-15T00:00:00Z",
      tracking_id_calendar: "tracking-cal-2",
      name: "Personal",
      enabled: true,
      provider: "google",
      source: "person@example.com",
      color: "#a142f4",
      connection_id: "conn-gmail",
    });

    store.setRow("events", "event-row", {
      user_id: "user-1",
      created_at: "2026-04-15T00:00:00Z",
      tracking_id_event: "event-1",
      calendar_id: "tracking-cal-2",
      title: "Standup",
      started_at: "2026-04-15T09:00:00Z",
      ended_at: "2026-04-15T09:30:00Z",
      provider: "google",
    });
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2026-04-15T00:00:00Z",
      title: "Standup notes",
      raw_md: "",
      event_json: JSON.stringify(
        makeSessionEvent({
          tracking_id: "event-1",
        }),
      ),
    });

    reconcileCalendarSessions(store);

    const session = store.getRow("sessions", "session-1");
    const embeddedEvent = JSON.parse(
      String(session?.event_json ?? ""),
    ) as SessionEvent;

    expect(embeddedEvent.calendar_id).toBe("tracking-cal-2");
  });
});
