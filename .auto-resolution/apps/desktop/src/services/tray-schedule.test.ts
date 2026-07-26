import { describe, expect, test } from "vitest";

import { buildTrayScheduleEvents } from "./tray-schedule";

import type { TimelineEventRow } from "~/sidebar/timeline/utils";

const NOW = Date.parse("2026-07-17T00:00:00.000Z");

function event(overrides: Partial<TimelineEventRow>): TimelineEventRow {
  return {
    has_recurrence_rules: false,
    ...overrides,
  };
}

describe("buildTrayScheduleEvents", () => {
  test("publishes active and upcoming timed events in start order", () => {
    const events = buildTrayScheduleEvents(
      {
        upcoming: event({
          title: "  Design sync  ",
          started_at: "2026-07-17T01:00:00.000Z",
          ended_at: "2026-07-17T02:00:00.000Z",
        }),
        active: event({
          title: "Standup",
          started_at: "2026-07-16T23:55:00.000Z",
          ended_at: "2026-07-17T00:25:00.000Z",
        }),
      },
      () => false,
      NOW,
      "UTC",
      "en-US",
    );

    expect(events).toEqual([
      {
        title: "Standup",
        startsAtMs: Date.parse("2026-07-16T23:55:00.000Z"),
        endsAtMs: Date.parse("2026-07-17T00:25:00.000Z"),
        dayStartMs: Date.parse("2026-07-16T00:00:00.000Z"),
        previousDayStartMs: Date.parse("2026-07-15T00:00:00.000Z"),
        timeLabel: "11:55 PM – 12:25 AM",
      },
      {
        title: "Design sync",
        startsAtMs: Date.parse("2026-07-17T01:00:00.000Z"),
        endsAtMs: Date.parse("2026-07-17T02:00:00.000Z"),
        dayStartMs: NOW,
        previousDayStartMs: Date.parse("2026-07-16T00:00:00.000Z"),
        timeLabel: "1:00 AM – 2:00 AM",
      },
    ]);
  });

  test("omits ignored, all-day, expired, and distant events", () => {
    const events = buildTrayScheduleEvents(
      {
        ignored: event({
          title: "Ignored",
          tracking_id_event: "ignored",
          started_at: "2026-07-17T01:00:00.000Z",
        }),
        allDay: event({
          title: "Holiday",
          is_all_day: true,
          started_at: "2026-07-17T00:00:00.000Z",
        }),
        expired: event({
          title: "Finished",
          started_at: "2026-07-16T22:00:00.000Z",
          ended_at: "2026-07-16T23:00:00.000Z",
        }),
        distant: event({
          title: "Next month",
          started_at: "2026-08-17T01:00:00.000Z",
        }),
      },
      (trackingId) => trackingId === "ignored",
      NOW,
      "UTC",
      "en-US",
    );

    expect(events).toEqual([]);
  });

  test("labels tomorrow in the configured timezone", () => {
    const events = buildTrayScheduleEvents(
      {
        tomorrow: event({
          title: "Planning",
          started_at: "2026-07-18T01:00:00.000Z",
          ended_at: "2026-07-18T02:00:00.000Z",
        }),
      },
      () => false,
      NOW,
      "UTC",
      "en-US",
    );

    expect(events[0]).toMatchObject({
      dayStartMs: Date.parse("2026-07-18T00:00:00.000Z"),
      previousDayStartMs: NOW,
      timeLabel: "1:00 AM – 2:00 AM",
    });
  });
});
