import { describe, expect, test } from "vitest";

import type { Ctx } from "../../ctx";
import type { ExistingEvent, IncomingEvent } from "../../fetch/types";
import { syncEvents } from "./sync";
import type { EventsSyncInput } from "./types";

function createMockStore(config: {
  eventToSession?: Map<string, string>;
  nonEmptySessions?: Set<string>;
}) {
  const eventToSession = config.eventToSession ?? new Map();
  const nonEmptySessions = config.nonEmptySessions ?? new Set();

  const sessionToEvent = new Map<string, string>();
  for (const [eventId, sessionId] of eventToSession) {
    sessionToEvent.set(sessionId, eventId);
  }

  return {
    getRow: (table: string, id: string) => {
      if (table === "sessions") {
        const eventId = sessionToEvent.get(id);
        if (!eventId) return {};
        const hasContent = nonEmptySessions.has(id);
        return {
          event_id: eventId,
          raw_md: hasContent ? "some content" : "",
        };
      }
      return {};
    },
    forEachRow: (table: string, callback: (rowId: string) => void) => {
      if (table === "sessions") {
        for (const sessionId of sessionToEvent.keys()) {
          callback(sessionId);
        }
      }
    },
  } as unknown as Ctx["store"];
}

function createMockCtx(
  overrides: Partial<Ctx> & {
    eventToSession?: Map<string, string>;
    nonEmptySessions?: Set<string>;
  } = {},
): Ctx {
  const store = createMockStore({
    eventToSession: overrides.eventToSession,
    nonEmptySessions: overrides.nonEmptySessions,
  });

  return {
    provider: "apple" as const,
    userId: "user-1",
    from: new Date("2024-01-01"),
    to: new Date("2024-02-01"),
    calendarIds: overrides.calendarIds ?? new Set(["cal-1"]),
    calendarTrackingIdToId:
      overrides.calendarTrackingIdToId ??
      new Map([["tracking-cal-1", "cal-1"]]),
    store,
    ...overrides,
  };
}

function createIncomingEvent(
  overrides: Partial<IncomingEvent> = {},
): IncomingEvent {
  return {
    tracking_id_event: "incoming-1",
    tracking_id_calendar: "tracking-cal-1",
    title: "Test Event",
    started_at: "2024-01-15T10:00:00Z",
    ended_at: "2024-01-15T11:00:00Z",
    has_recurrence_rules: false,
    is_all_day: false,
    ...overrides,
  };
}

function createExistingEvent(
  overrides: Partial<ExistingEvent> = {},
): ExistingEvent {
  return {
    id: "event-1",
    tracking_id_event: "existing-1",
    calendar_id: "cal-1",
    user_id: "user-1",
    created_at: "2024-01-01T00:00:00Z",
    title: "Existing Event",
    started_at: "2024-01-15T10:00:00Z",
    ended_at: "2024-01-15T11:00:00Z",
    provider: "apple",
    ...overrides,
  };
}

function syncInput(overrides: Partial<EventsSyncInput> = {}): EventsSyncInput {
  return {
    incoming: [],
    existing: [],
    incomingParticipants: new Map(),
    ...overrides,
  };
}

describe("syncEvents", () => {
  test("adds new incoming events", () => {
    const ctx = createMockCtx();
    const result = syncEvents(
      ctx,
      syncInput({
        incoming: [createIncomingEvent()],
      }),
    );

    expect(result.toAdd).toHaveLength(1);
    expect(result.toDelete).toHaveLength(0);
    expect(result.toUpdate).toHaveLength(0);
  });

  test("deletes events from disabled calendars", () => {
    const ctx = createMockCtx({ calendarIds: new Set(["cal-2"]) });
    const result = syncEvents(
      ctx,
      syncInput({
        existing: [createExistingEvent()],
      }),
    );

    expect(result.toDelete).toContain("event-1");
  });

  test("updates existing events with matching tracking id", () => {
    const ctx = createMockCtx();
    const result = syncEvents(
      ctx,
      syncInput({
        incoming: [createIncomingEvent({ tracking_id_event: "existing-1" })],
        existing: [createExistingEvent()],
      }),
    );

    expect(result.toUpdate).toHaveLength(1);
    expect(result.toAdd).toHaveLength(0);
    expect(result.toDelete).toHaveLength(0);
  });

  test("deletes orphaned events without matching incoming", () => {
    const ctx = createMockCtx();
    const result = syncEvents(
      ctx,
      syncInput({
        existing: [createExistingEvent()],
      }),
    );

    expect(result.toDelete).toContain("event-1");
  });

  describe("removed calendar cleanup", () => {
    test("deletes events when calendar removed from Apple Calendar (no incoming events)", () => {
      const ctx = createMockCtx({
        calendarIds: new Set(["cal-1"]),
        calendarTrackingIdToId: new Map([["tracking-cal-1", "cal-1"]]),
      });

      const result = syncEvents(
        ctx,
        syncInput({
          existing: [
            createExistingEvent({
              id: "event-1",
              tracking_id_event: "track-1",
            }),
            createExistingEvent({
              id: "event-2",
              tracking_id_event: "track-2",
            }),
          ],
        }),
      );

      expect(result.toDelete).toContain("event-1");
      expect(result.toDelete).toContain("event-2");
      expect(result.toDelete).toHaveLength(2);
    });

    test("deletes events regardless of non-empty sessions when calendar removed", () => {
      const ctx = createMockCtx({
        calendarIds: new Set(["cal-1"]),
        eventToSession: new Map([["event-1", "session-1"]]),
        nonEmptySessions: new Set(["session-1"]),
      });

      const result = syncEvents(
        ctx,
        syncInput({
          existing: [
            createExistingEvent({
              id: "event-1",
              tracking_id_event: "track-1",
            }),
            createExistingEvent({
              id: "event-2",
              tracking_id_event: "track-2",
            }),
          ],
        }),
      );

      expect(result.toDelete).toContain("event-1");
      expect(result.toDelete).toContain("event-2");
    });

    test("deletes events with empty sessions when calendar removed", () => {
      const ctx = createMockCtx({
        calendarIds: new Set(["cal-1"]),
        eventToSession: new Map([["event-1", "session-1"]]),
        nonEmptySessions: new Set(),
      });

      const result = syncEvents(
        ctx,
        syncInput({
          existing: [createExistingEvent({ id: "event-1" })],
        }),
      );

      expect(result.toDelete).toContain("event-1");
    });

    test("only deletes events from removed calendar, keeps events from active calendars", () => {
      const ctx = createMockCtx({
        calendarIds: new Set(["cal-1", "cal-2"]),
        calendarTrackingIdToId: new Map([
          ["tracking-cal-1", "cal-1"],
          ["tracking-cal-2", "cal-2"],
        ]),
      });

      const result = syncEvents(
        ctx,
        syncInput({
          incoming: [
            createIncomingEvent({
              tracking_id_event: "track-2",
              tracking_id_calendar: "tracking-cal-2",
            }),
          ],
          existing: [
            createExistingEvent({
              id: "event-1",
              calendar_id: "cal-1",
              tracking_id_event: "track-1",
            }),
            createExistingEvent({
              id: "event-2",
              calendar_id: "cal-2",
              tracking_id_event: "track-2",
            }),
          ],
        }),
      );

      expect(result.toDelete).toContain("event-1");
      expect(result.toDelete).not.toContain("event-2");
      expect(result.toUpdate).toHaveLength(1);
    });
  });

  describe("participants", () => {
    test("attaches participants to added events", () => {
      const ctx = createMockCtx();
      const participants = [
        { email: "alice@example.com", name: "Alice", is_organizer: true },
        { email: "bob@example.com", name: "Bob" },
      ];
      const result = syncEvents(
        ctx,
        syncInput({
          incoming: [createIncomingEvent()],
          incomingParticipants: new Map([["incoming-1", participants]]),
        }),
      );

      expect(result.toAdd).toHaveLength(1);
      expect(result.toAdd[0].participants).toEqual(participants);
    });

    test("attaches participants to updated events", () => {
      const ctx = createMockCtx();
      const participants = [{ email: "alice@example.com", name: "Alice" }];
      const result = syncEvents(
        ctx,
        syncInput({
          incoming: [createIncomingEvent({ tracking_id_event: "existing-1" })],
          existing: [createExistingEvent()],
          incomingParticipants: new Map([["existing-1", participants]]),
        }),
      );

      expect(result.toUpdate).toHaveLength(1);
      expect(result.toUpdate[0].participants).toEqual(participants);
    });

    test("defaults to empty participants when no match in incomingParticipants", () => {
      const ctx = createMockCtx();
      const result = syncEvents(
        ctx,
        syncInput({
          incoming: [createIncomingEvent()],
          incomingParticipants: new Map(),
        }),
      );

      expect(result.toAdd).toHaveLength(1);
      expect(result.toAdd[0].participants).toEqual([]);
    });

    test("matches participants by tracking_id_event for recurring events", () => {
      const ctx = createMockCtx();
      const participants = [{ email: "alice@example.com", name: "Alice" }];
      const result = syncEvents(
        ctx,
        syncInput({
          incoming: [
            createIncomingEvent({
              tracking_id_event: "recurring-1",
              has_recurrence_rules: true,
              started_at: "2024-01-15T10:00:00Z",
            }),
          ],
          incomingParticipants: new Map([["recurring-1", participants]]),
        }),
      );

      expect(result.toAdd).toHaveLength(1);
      expect(result.toAdd[0].participants).toEqual(participants);
    });
  });

  describe("disabled calendar cleanup", () => {
    test("deletes events regardless of non-empty sessions when calendar disabled", () => {
      const ctx = createMockCtx({
        calendarIds: new Set(["cal-2"]),
      });

      const result = syncEvents(
        ctx,
        syncInput({
          existing: [
            createExistingEvent({ id: "event-1", calendar_id: "cal-1" }),
          ],
        }),
      );

      expect(result.toDelete).toContain("event-1");
    });

    test("deletes events from disabled calendar without sessions", () => {
      const ctx = createMockCtx({
        calendarIds: new Set(["cal-2"]),
      });

      const result = syncEvents(
        ctx,
        syncInput({
          existing: [
            createExistingEvent({ id: "event-1", calendar_id: "cal-1" }),
          ],
        }),
      );

      expect(result.toDelete).toContain("event-1");
    });
  });
});
