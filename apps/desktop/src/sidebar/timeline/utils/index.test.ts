import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  buildTimelineBuckets,
  calculateTodayIndicatorPlacement,
  getBucketInfo,
  type TimelineEventsTable,
  type TimelineSessionsTable,
} from ".";

process.env.TZ = "UTC";

const SYSTEM_TIME = new Date("2024-01-15T12:00:00.000Z");

describe("timeline utils", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(SYSTEM_TIME);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("getBucketInfo returns Today for current date", () => {
    const info = getBucketInfo(new Date("2024-01-15T05:00:00.000Z"));
    expect(info).toMatchObject({ label: "Today", precision: "time" });
  });

  test("getBucketInfo groups recent past days", () => {
    const info = getBucketInfo(new Date("2024-01-10T05:00:00.000Z"));
    expect(info).toMatchObject({ label: "5 days ago", precision: "time" });
  });

  test("getBucketInfo groups distant future months", () => {
    const info = getBucketInfo(new Date("2024-03-20T12:00:00.000Z"));
    expect(info).toMatchObject({ label: "in 2 months", precision: "date" });
  });

  test("calculateTodayIndicatorPlacement places indicator inside an active timed session", () => {
    const placement = calculateTodayIndicatorPlacement(
      [
        {
          item: {
            type: "session",
            id: "session-1",
            data: {
              title: "test",
              created_at: "2024-01-15T11:30:00.000Z",
              event_json: JSON.stringify({
                started_at: "2024-01-15T11:30:00.000Z",
                ended_at: "2024-01-15T12:30:00.000Z",
              }),
            },
          },
          timestamp: new Date("2024-01-15T11:30:00.000Z"),
        },
      ],
      new Date("2024-01-15T12:00:00.000Z"),
    );

    expect(placement).toMatchObject({
      type: "inside",
      index: 0,
      progress: 0.5,
    });
  });

  test("calculateTodayIndicatorPlacement falls back to seam placement for future-only items", () => {
    const placement = calculateTodayIndicatorPlacement(
      [
        {
          item: {
            type: "event",
            id: "event-1",
            data: {
              title: "Future Event",
              started_at: "2024-01-15T13:00:00.000Z",
              ended_at: "2024-01-15T14:00:00.000Z",
              has_recurrence_rules: false,
            },
          },
          timestamp: new Date("2024-01-15T13:00:00.000Z"),
        },
      ],
      new Date("2024-01-15T12:00:00.000Z"),
    );

    expect(placement).toEqual({ type: "after" });
  });

  test("buildTimelineBuckets excludes Today bucket when empty", () => {
    const buckets = buildTimelineBuckets({
      timelineEventsTable: null,
      timelineSessionsTable: null,
    });

    const todayBucket = buckets.find((bucket) => bucket.label === "Today");
    expect(todayBucket).toBeUndefined();
  });

  test("buildTimelineBuckets prioritizes sessions to events and avoid duplicate timeline items", () => {
    const timelineEventsTable: TimelineEventsTable = {
      "event-1": {
        title: "Future Event",
        started_at: "2024-01-18T12:00:00.000Z",
        ended_at: "2024-01-18T13:00:00.000Z",
        calendar_id: "cal-1",
        tracking_id_event: "event-1",
        has_recurrence_rules: false,
      },
    };

    const timelineSessionsTable: TimelineSessionsTable = {
      "session-1": {
        title: "Linked Session",
        created_at: "2024-01-10T12:00:00.000Z",
        event_json: JSON.stringify({
          tracking_id: "event-1",
          started_at: "2024-01-18T12:00:00.000Z",
        }),
      },
      "session-2": {
        title: "Standalone Session",
        created_at: "2024-01-14T12:00:00.000Z",
      },
    };

    const buckets = buildTimelineBuckets({
      timelineEventsTable,
      timelineSessionsTable,
    });

    const futureBucket = buckets[0];
    expect(futureBucket.label).toBe("in 3 days");
    expect(futureBucket.items).toHaveLength(1);
    expect(futureBucket.items[0]).toMatchObject({
      type: "session",
      id: "session-1",
    });

    const sessionBucket = buckets.find((bucket) =>
      bucket.items.some((item) => item.id === "session-2"),
    );
    expect(sessionBucket).toBeDefined();
    expect(sessionBucket?.items).toHaveLength(1);
    const containsLinkedEvent = buckets.some((bucket) =>
      bucket.items.some((item) => item.id === "event-1"),
    );
    expect(containsLinkedEvent).toBe(false);
  });

  test("buildTimelineBuckets excludes past events but keeps related sessions", () => {
    const timelineEventsTable: TimelineEventsTable = {
      "event-past": {
        title: "Past Event",
        started_at: "2024-01-10T10:00:00.000Z",
        ended_at: "2024-01-10T11:00:00.000Z",
        calendar_id: "cal-1",
        tracking_id_event: "event-past",
        has_recurrence_rules: false,
      },
    };

    const timelineSessionsTable: TimelineSessionsTable = {
      "session-past": {
        title: "Follow-up Session",
        created_at: "2024-01-10T12:00:00.000Z",
        event_json: JSON.stringify({
          tracking_id: "event-past",
          started_at: "2024-01-10T10:00:00.000Z",
        }),
      },
    };

    const buckets = buildTimelineBuckets({
      timelineEventsTable,
      timelineSessionsTable,
    });

    const pastBucket = buckets.find((bucket) => bucket.label === "5 days ago");
    expect(pastBucket).toBeDefined();
    expect(pastBucket?.items).toHaveLength(1);
    expect(pastBucket?.items[0]).toMatchObject({
      type: "session",
      id: "session-past",
    });

    const hasPastEvent = buckets.some((bucket) =>
      bucket.items.some((item) => item.id === "event-past"),
    );
    expect(hasPastEvent).toBe(false);

    const todayBucket = buckets.find((bucket) => bucket.label === "Today");
    expect(todayBucket).toBeUndefined();
  });

  test("buildTimelineBuckets sorts buckets by most recent first", () => {
    const timelineSessionsTable: TimelineSessionsTable = {
      "session-future": {
        title: "Future Session",
        created_at: "2024-01-10T12:00:00.000Z",
        event_json: JSON.stringify({ started_at: "2024-01-16T09:00:00.000Z" }),
      },
      "session-past": {
        title: "Past Session",
        created_at: "2024-01-14T09:00:00.000Z",
      },
    };

    const buckets = buildTimelineBuckets({
      timelineEventsTable: null,
      timelineSessionsTable,
    });

    expect(buckets.map((bucket) => bucket.label)).toEqual([
      "Tomorrow",
      "Yesterday",
    ]);
  });

  test("getBucketInfo: future month bucket sorts after all week buckets", () => {
    // System time is 2024-01-15
    // Week buckets: absDays <= 27, Month buckets: absDays > 27
    // "in 4 weeks" = ~25-27 days, "next month" = 28+ days
    const in4Weeks = getBucketInfo(new Date("2024-02-11T12:00:00.000Z")); // 27 days out (last day of week bucket)
    const nextMonth = getBucketInfo(new Date("2024-02-13T12:00:00.000Z")); // 29 days out (first day of month bucket)

    expect(in4Weeks.label).toBe("in 4 weeks");
    expect(nextMonth.label).toBe("next month");
    expect(nextMonth.sortKey).toBeGreaterThan(in4Weeks.sortKey);
  });

  test("getBucketInfo: past month bucket sorts before all week buckets", () => {
    // Week buckets: absDays <= 27, Month buckets: absDays > 27
    // "4 weeks ago" = ~25-27 days ago, "a month ago" = 28+ days ago
    const weeksAgo4 = getBucketInfo(new Date("2023-12-19T12:00:00.000Z")); // 27 days ago (last day of week bucket)
    const monthAgo = getBucketInfo(new Date("2023-12-17T12:00:00.000Z")); // 29 days ago (first day of month bucket)

    expect(weeksAgo4.label).toBe("4 weeks ago");
    expect(monthAgo.label).toBe("a month ago");
    expect(monthAgo.sortKey).toBeLessThan(weeksAgo4.sortKey);
  });

  test("buildTimelineBuckets deduplicates recurring events by tracking_id", () => {
    const timelineEventsTable: TimelineEventsTable = {
      "event-jan18": {
        title: "Weekly Standup",
        started_at: "2024-01-18T09:00:00.000Z",
        ended_at: "2024-01-18T09:30:00.000Z",
        calendar_id: "cal-1",
        tracking_id_event: "recurring-1:2024-01-18",
        has_recurrence_rules: true,
        recurrence_series_id: "series-1",
      },
      "event-jan25": {
        title: "Weekly Standup",
        started_at: "2024-01-25T09:00:00.000Z",
        ended_at: "2024-01-25T09:30:00.000Z",
        calendar_id: "cal-1",
        tracking_id_event: "recurring-1:2024-01-25",
        has_recurrence_rules: true,
        recurrence_series_id: "series-1",
      },
    };

    const timelineSessionsTable: TimelineSessionsTable = {
      "session-jan18": {
        title: "Weekly Standup",
        created_at: "2024-01-18T09:00:00.000Z",
        event_json: JSON.stringify({
          tracking_id: "recurring-1:2024-01-18",
          started_at: "2024-01-18T09:00:00.000Z",
          has_recurrence_rules: true,
        }),
      },
    };

    const buckets = buildTimelineBuckets({
      timelineEventsTable,
      timelineSessionsTable,
    });

    const allItems = buckets.flatMap((b) => b.items);

    const jan18Session = allItems.find(
      (i) => i.type === "session" && i.id === "session-jan18",
    );
    expect(jan18Session).toBeDefined();

    const jan18Event = allItems.find(
      (i) => i.type === "event" && i.id === "event-jan18",
    );
    expect(jan18Event).toBeUndefined();

    const jan25Event = allItems.find(
      (i) => i.type === "event" && i.id === "event-jan25",
    );
    expect(jan25Event).toBeDefined();
  });

  test("buildTimelineBuckets does not deduplicate recurring events with different tracking_ids", () => {
    const timelineEventsTable: TimelineEventsTable = {
      "event-jan18": {
        title: "Weekly Standup",
        started_at: "2024-01-18T09:00:00.000Z",
        ended_at: "2024-01-18T09:30:00.000Z",
        calendar_id: "cal-1",
        tracking_id_event: "recurring-1:2024-01-18",
        has_recurrence_rules: true,
      },
      "event-jan25": {
        title: "Weekly Standup",
        started_at: "2024-01-25T09:00:00.000Z",
        ended_at: "2024-01-25T09:30:00.000Z",
        calendar_id: "cal-1",
        tracking_id_event: "recurring-1:2024-01-25",
        has_recurrence_rules: true,
      },
    };

    const buckets = buildTimelineBuckets({
      timelineEventsTable,
      timelineSessionsTable: null,
    });

    const allItems = buckets.flatMap((b) => b.items);
    const eventItems = allItems.filter((i) => i.type === "event");
    expect(eventItems).toHaveLength(2);
  });

  test("buildTimelineBuckets: future buckets sort correctly (weeks before months)", () => {
    const timelineSessionsTable: TimelineSessionsTable = {
      "session-2weeks": {
        title: "In 2 weeks",
        event_json: JSON.stringify({ started_at: "2024-01-29T09:00:00.000Z" }), // 14 days -> "in 2 weeks"
        created_at: "2024-01-10T12:00:00.000Z",
      },
      "session-4weeks": {
        title: "In 4 weeks",
        event_json: JSON.stringify({ started_at: "2024-02-11T09:00:00.000Z" }), // 27 days -> "in 4 weeks"
        created_at: "2024-01-10T12:00:00.000Z",
      },
      "session-nextmonth": {
        title: "Next month",
        event_json: JSON.stringify({ started_at: "2024-02-13T09:00:00.000Z" }), // 29 days -> "next month"
        created_at: "2024-01-10T12:00:00.000Z",
      },
    };

    const buckets = buildTimelineBuckets({
      timelineEventsTable: null,
      timelineSessionsTable,
    });

    // Should be: next month, in 4 weeks, in 2 weeks (furthest future first)
    expect(buckets.map((b) => b.label)).toEqual([
      "next month",
      "in 4 weeks",
      "in 2 weeks",
    ]);
  });
});
