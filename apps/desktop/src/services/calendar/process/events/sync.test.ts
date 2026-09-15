import { describe, expect, test } from "vitest";

import type { Ctx } from "../../ctx";
import type { ExistingEvent, IncomingEvent } from "../../fetch/types";
import { syncEvents } from "./sync";
import type { EventsSyncInput } from "./types";

function createMockCtx(
  overrides: Partial<Ctx> & {
    eventToSession?: Map<string, string>;
    nonEmptySessions?: Set<string>;
  } = {},
): Ctx {
  const {
    eventToSession: _eventToSession,
    nonEmptySessions: _sessions,
    ...ctx
  } = overrides;

  return {
    provider: "apple" as const,
    connectionId: "apple",
    from: new Date("2024-01-01"),
    to: new Date("2024-02-01"),
    calendarIds: overrides.calendarIds ?? new Set(["cal-1"]),
    calendarTrackingIdToId:
      overrides.calendarTrackingIdToId ??
      new Map([["tracking-cal-1", "cal-1"]]),
    ...ctx,
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
    created_at: "2024-01-01T00:00:00Z",
    title: "Existing Event",
    started_at: "2024-01-15T10:00:00Z",
    ended_at: "2024-01-15T11:00:00Z",
    location: "",
    meeting_link: "",
    description: "",
    note: "",
    recurrence_series_id: "",
    has_recurrence_rules: false,
    is_all_day: false,
    provider: "apple",
    deleted_at: null,
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

  test("resurrects a tombstoned event instead of allocating a new id", () => {
    const result = syncEvents(
      createMockCtx(),
      syncInput({
        incoming: [createIncomingEvent({ tracking_id_event: "existing-1" })],
        existing: [
          createExistingEvent({
            deleted_at: "2024-01-10T00:00:00Z",
          }),
        ],
      }),
    );

    expect(result.toUpdate.map((event) => event.id)).toEqual(["event-1"]);
    expect(result.toAdd).toEqual([]);
    expect(result.toDelete).toEqual([]);
  });

  test("keeps one durable row when duplicate active events exist", () => {
    const result = syncEvents(
      createMockCtx(),
      syncInput({
        incoming: [createIncomingEvent({ tracking_id_event: "existing-1" })],
        existing: [
          createExistingEvent({ id: "event-1" }),
          createExistingEvent({ id: "event-duplicate" }),
        ],
      }),
    );

    expect(result.toUpdate.map((event) => event.id)).toEqual(["event-1"]);
    expect(result.toDelete).toEqual(["event-duplicate"]);
    expect(result.toAdd).toEqual([]);
  });

  test("keeps the first existing row for an exact visible Apple occurrence", () => {
    const result = syncEvents(
      createMockCtx(),
      syncInput({
        incoming: [
          createIncomingEvent({
            tracking_id_event: "current-series:2024-01-15",
            recurrence_series_id: "current-series",
            has_recurrence_rules: true,
            title: "Team planning",
            provider_modified_at: "2024-01-12T00:00:00Z",
          }),
        ],
        existing: [
          createExistingEvent({
            id: "event-keeper",
            tracking_id_event: "old-series:2024-01-15",
            recurrence_series_id: "old-series",
            has_recurrence_rules: true,
            title: "Team planning",
            deleted_at: "2024-01-10T00:00:00Z",
          }),
          createExistingEvent({
            id: "event-duplicate",
            tracking_id_event: "other-series:2024-01-15",
            recurrence_series_id: "other-series",
            has_recurrence_rules: true,
            title: "Team planning",
          }),
        ],
      }),
    );

    expect(result.toUpdate).toHaveLength(1);
    expect(result.toUpdate[0]).toMatchObject({
      id: "event-keeper",
      tracking_id_event: "current-series:2024-01-15",
      title: "Team planning",
    });
    expect(result.toDelete).toEqual(["event-duplicate"]);
    expect(result.toAdd).toEqual([]);
  });

  test("chooses the newest provider record for duplicate incoming events", () => {
    const result = syncEvents(
      createMockCtx(),
      syncInput({
        incoming: [
          createIncomingEvent({
            tracking_id_event: "current-series:2024-01-15",
            title: "Team planning",
            has_recurrence_rules: true,
            meeting_link: "https://meet.example/current",
            provider_modified_at: "2024-01-12T00:00:00Z",
          }),
          createIncomingEvent({
            tracking_id_event: "old-series:2024-01-15",
            title: "Team planning",
            has_recurrence_rules: true,
            meeting_link: "https://meet.example/old",
            provider_modified_at: "2024-01-01T00:00:00Z",
          }),
        ],
      }),
    );

    expect(result.toAdd.map((event) => event.tracking_id_event)).toEqual([
      "current-series:2024-01-15",
    ]);
    expect(result.toAdd[0].meeting_link).toBe("https://meet.example/current");
  });

  test("migrates a stale stored row through a losing tracking alias", () => {
    const result = syncEvents(
      createMockCtx(),
      syncInput({
        incoming: [
          createIncomingEvent({
            tracking_id_event: "current-series",
            title: "Team planning",
            provider_modified_at: "2024-01-12T00:00:00Z",
          }),
          createIncomingEvent({
            tracking_id_event: "old-series",
            title: "Team planning",
            provider_modified_at: "2024-01-01T00:00:00Z",
          }),
        ],
        existing: [
          createExistingEvent({
            id: "stale-row",
            tracking_id_event: "old-series",
            title: "Stale embedded title",
          }),
        ],
      }),
    );

    expect(result.toUpdate).toHaveLength(1);
    expect(result.toUpdate[0]).toMatchObject({
      id: "stale-row",
      tracking_id_event: "current-series",
      title: "Team planning",
    });
    expect(result.toAdd).toEqual([]);
    expect(result.toDelete).toEqual([]);
  });

  test("replays the nine live duplicate groups without row growth", () => {
    const groupSizes = [2, 4, 3, 2, 2, 3, 3, 2, 3];
    const incoming = groupSizes.flatMap((size, groupIndex) =>
      Array.from({ length: size }, (_, variantIndex) =>
        createIncomingEvent({
          tracking_id_event: `group-${groupIndex}-series-${variantIndex}`,
          title: `Captured meeting ${groupIndex}`,
          started_at: `2024-01-${String(groupIndex + 10).padStart(2, "0")}T10:00:00Z`,
          ended_at: `2024-01-${String(groupIndex + 10).padStart(2, "0")}T11:00:00Z`,
          has_recurrence_rules: true,
          provider_modified_at: `2024-01-${String(variantIndex + 1).padStart(2, "0")}T00:00:00Z`,
        }),
      ),
    );

    const existing = incoming.map((event, index) =>
      createExistingEvent({
        ...event,
        id: `event-${index}`,
        calendar_id: "cal-1",
        title: event.title ?? "",
        started_at: event.started_at ?? "",
        ended_at: event.ended_at ?? "",
      }),
    );
    const first = syncEvents(
      createMockCtx(),
      syncInput({ incoming, existing }),
    );
    expect(first.toAdd).toEqual([]);
    expect(first.toUpdate).toHaveLength(9);
    expect(first.toDelete).toHaveLength(15);

    const second = syncEvents(
      createMockCtx(),
      syncInput({ incoming, existing: first.toUpdate }),
    );

    expect(second.toAdd).toEqual([]);
    expect(second.toDelete).toEqual([]);
    expect(second.toUpdate.map((event) => event.id)).toEqual(
      first.toUpdate.map((event) => event.id),
    );
  });

  test("keeps visibly different Apple occurrences distinct", () => {
    const result = syncEvents(
      createMockCtx(),
      syncInput({
        incoming: [
          createIncomingEvent({
            tracking_id_event: "series-a:2024-01-15",
            title: "Team planning",
            has_recurrence_rules: true,
          }),
          createIncomingEvent({
            tracking_id_event: "series-b:2024-01-15",
            title: "Team planning",
            ended_at: "2024-01-15T11:30:00Z",
            has_recurrence_rules: true,
          }),
        ],
      }),
    );

    expect(result.toAdd.map((event) => event.tracking_id_event)).toEqual([
      "series-a:2024-01-15",
      "series-b:2024-01-15",
    ]);
  });

  test("keeps matching-looking Apple occurrences from different calendars", () => {
    const result = syncEvents(
      createMockCtx({
        calendarIds: new Set(["cal-1", "cal-2"]),
        calendarTrackingIdToId: new Map([
          ["tracking-cal-1", "cal-1"],
          ["tracking-cal-2", "cal-2"],
        ]),
      }),
      syncInput({
        incoming: [
          createIncomingEvent({
            tracking_id_event: "calendar-1-event",
            title: "Team planning",
          }),
          createIncomingEvent({
            tracking_id_event: "calendar-2-event",
            tracking_id_calendar: "tracking-cal-2",
            title: "Team planning",
          }),
        ],
      }),
    );

    expect(result.toAdd).toHaveLength(2);
  });

  test("deletes an out-of-window row when its tracking id moved calendars", () => {
    const result = syncEvents(
      createMockCtx({
        calendarIds: new Set(["cal-1", "cal-2"]),
        calendarTrackingIdToId: new Map([
          ["tracking-cal-1", "cal-1"],
          ["tracking-cal-2", "cal-2"],
        ]),
      }),
      syncInput({
        incoming: [
          createIncomingEvent({
            tracking_id_event: "moved-event",
            tracking_id_calendar: "tracking-cal-2",
          }),
        ],
        existing: [
          createExistingEvent({
            id: "old-calendar-row",
            tracking_id_event: "moved-event",
            calendar_id: "cal-1",
            started_at: "2026-09-01T18:00:00Z",
            ended_at: "2026-09-01T19:00:00Z",
          }),
        ],
      }),
    );

    expect(result.toDelete).toEqual(["old-calendar-row"]);
    expect(result.toAdd).toHaveLength(1);
  });

  test("coalesces untitled Apple occurrences with the same exact times", () => {
    const result = syncEvents(
      createMockCtx(),
      syncInput({
        incoming: [
          createIncomingEvent({
            tracking_id_event: "untitled-old",
            title: "   ",
          }),
          createIncomingEvent({
            tracking_id_event: "untitled-new",
            title: "",
          }),
        ],
      }),
    );

    expect(result.toAdd).toHaveLength(1);
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

  test("does not delete an unmatched alias candidate outside the sync window", () => {
    const result = syncEvents(
      createMockCtx(),
      syncInput({
        incoming: [],
        existing: [
          createExistingEvent({
            id: "event-outside-window",
            started_at: "2026-09-01T18:00:00Z",
            ended_at: "2026-09-01T19:00:00Z",
          }),
        ],
      }),
    );

    expect(result.toDelete).toEqual([]);
  });
});
