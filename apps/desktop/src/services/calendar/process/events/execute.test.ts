import { describe, expect, test } from "vitest";

import type { SessionEvent } from "@anlg/store";

import type { Ctx } from "../../ctx";
import type { IncomingEvent } from "../../fetch/types";
import type { SessionSyncRow } from "../../storage";
import { syncSessionEmbeddedEvents } from "./execute";

function createMockCtx(overrides: Partial<Ctx> = {}): Ctx {
  return {
    provider: "apple",
    connectionId: "apple",
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

function makeSession(
  id: string,
  event: SessionEvent = makeSessionEvent(),
): SessionSyncRow {
  return {
    id,
    ownerUserId: "user-1",
    eventJson: JSON.stringify(event),
    trackingId: event.tracking_id,
    calendarId: event.calendar_id,
    title: event.title,
    startedAt: event.started_at,
    endedAt: event.ended_at,
    isAllDay: event.is_all_day,
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
  test("builds an update for a matching non-recurring event", () => {
    const updates = syncSessionEmbeddedEvents(
      createMockCtx(),
      [makeIncomingEvent({ title: "Updated Title" })],
      [makeSession("session-1")],
    );

    expect(updates).toHaveLength(1);
    const event = JSON.parse(updates[0].eventJson);
    expect(event.title).toBe("Updated Title");
    expect(event.tracking_id).toBe("track-1");
  });

  test("matches recurring events by occurrence tracking id", () => {
    const jan15 = makeSession(
      "session-jan15",
      makeSessionEvent({ tracking_id: "recurring-1:2024-01-15" }),
    );
    const jan22 = makeSession(
      "session-jan22",
      makeSessionEvent({ tracking_id: "recurring-1:2024-01-22" }),
    );

    const updates = syncSessionEmbeddedEvents(
      createMockCtx(),
      [
        makeIncomingEvent({
          tracking_id_event: "recurring-1:2024-01-15",
          has_recurrence_rules: true,
          title: "Updated Jan 15",
        }),
      ],
      [jan15, jan22],
    );

    expect(updates.map((update) => update.sessionId)).toEqual([
      "session-jan15",
    ]);
  });

  test("preserves a session when EventKit replaces its recurring series id", () => {
    const updates = syncSessionEmbeddedEvents(
      createMockCtx(),
      [
        makeIncomingEvent({
          tracking_id_event: "external-1:old-series:2024-01-15",
          recurrence_series_id: "old-series",
          has_recurrence_rules: true,
          title: "Team planning",
          provider_modified_at: "2024-01-01T00:00:00Z",
        }),
        makeIncomingEvent({
          tracking_id_event: "external-1:new-series:2024-01-15",
          recurrence_series_id: "new-series",
          has_recurrence_rules: true,
          title: "Team planning",
          provider_modified_at: "2024-01-12T00:00:00Z",
        }),
      ],
      [
        {
          ...makeSession(
            "session-1",
            makeSessionEvent({
              tracking_id: "external-1:old-series:2024-01-15",
              title: "Stale embedded title",
              recurrence_series_id: "old-series",
              has_recurrence_rules: true,
            }),
          ),
          calendarId: "cal-1",
        },
      ],
    );

    expect(updates).toHaveLength(1);
    expect(updates[0].trackingId).toBe("external-1:new-series:2024-01-15");
    expect(JSON.parse(updates[0].eventJson)).toMatchObject({
      tracking_id: "external-1:new-series:2024-01-15",
      title: "Team planning",
    });
  });

  test("migrates a session attached to a detached EventKit occurrence", () => {
    const updates = syncSessionEmbeddedEvents(
      createMockCtx(),
      [
        makeIncomingEvent({
          tracking_id_event: "external-1:2026-09-14",
          has_recurrence_rules: false,
          title: "Team planning",
          started_at: "2026-09-16T05:00:00Z",
          ended_at: "2026-09-16T06:00:00Z",
        }),
      ],
      [
        {
          ...makeSession(
            "session-1",
            makeSessionEvent({
              tracking_id: "external-1:series-b/RID=811141200",
              title: "Team planning",
              started_at: "2026-09-16T05:00:00Z",
              ended_at: "2026-09-16T06:00:00Z",
            }),
          ),
        },
      ],
    );

    expect(updates).toHaveLength(1);
    expect(updates[0].trackingId).toBe("external-1:2026-09-14");
  });

  test("does not fall back to an event from another known calendar", () => {
    const updates = syncSessionEmbeddedEvents(
      createMockCtx({
        calendarTrackingIdToId: new Map([
          ["tracking-cal-1", "cal-1"],
          ["tracking-cal-2", "cal-2"],
        ]),
      }),
      [
        makeIncomingEvent({
          tracking_id_event: "shared-tracking-id",
          tracking_id_calendar: "tracking-cal-2",
        }),
      ],
      [
        {
          ...makeSession(
            "session-1",
            makeSessionEvent({ tracking_id: "shared-tracking-id" }),
          ),
          calendarId: "cal-1",
        },
      ],
    );

    expect(updates).toEqual([]);
  });

  test("does not fall back when the session calendar was removed", () => {
    const updates = syncSessionEmbeddedEvents(
      createMockCtx({
        calendarIds: new Set(["cal-2"]),
        calendarTrackingIdToId: new Map([["tracking-cal-2", "cal-2"]]),
      }),
      [
        makeIncomingEvent({
          tracking_id_event: "shared-tracking-id",
          tracking_id_calendar: "tracking-cal-2",
        }),
      ],
      [
        {
          ...makeSession(
            "session-1",
            makeSessionEvent({ tracking_id: "shared-tracking-id" }),
          ),
          calendarId: "cal-removed",
        },
      ],
    );

    expect(updates).toEqual([]);
  });

  test("skips sessions without a matching event", () => {
    const updates = syncSessionEmbeddedEvents(
      createMockCtx(),
      [makeIncomingEvent()],
      [
        {
          id: "session-1",
          ownerUserId: "user-1",
          eventJson: "",
          trackingId: "other-event",
          calendarId: "cal-1",
          title: "",
          startedAt: "",
          endedAt: "",
          isAllDay: false,
        },
      ],
    );

    expect(updates).toEqual([]);
  });

  test("does nothing when incoming events are empty", () => {
    expect(
      syncSessionEmbeddedEvents(
        createMockCtx(),
        [],
        [makeSession("session-1")],
      ),
    ).toEqual([]);
  });

  test("resolves the canonical calendar id", () => {
    const updates = syncSessionEmbeddedEvents(
      createMockCtx({
        calendarIds: new Set(["cal-new"]),
        calendarTrackingIdToId: new Map([["tracking-cal-new", "cal-new"]]),
      }),
      [makeIncomingEvent({ tracking_id_calendar: "tracking-cal-new" })],
      [{ ...makeSession("session-1"), calendarId: "" }],
    );

    expect(JSON.parse(updates[0].eventJson).calendar_id).toBe("cal-new");
  });
});
