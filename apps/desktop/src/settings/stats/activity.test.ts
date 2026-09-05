import { describe, expect, it } from "vitest";

import { summarizeActivity } from "./activity";
import type { ActivityRecord } from "./queries";

const now = new Date("2026-09-05T12:00:00Z");
const record = (
  session: string,
  date: string,
  duration = 3_600_000,
): ActivityRecord => ({
  session_id: session,
  started_at_ms: Date.parse(date),
  created_at: date,
  duration_ms: duration,
});

describe("personal activity", () => {
  it("starts empty with a first-conversation milestone and a complete calendar", () => {
    const stats = summarizeActivity([], now, "UTC");
    expect(stats).toMatchObject({
      conversations: 0,
      activeDays: 0,
      hours: 0,
      streak: 0,
      nextMilestone: 1,
    });
    expect(stats.days[0].date.getDay()).toBe(0);
    expect(stats.days[stats.days.length - 1]?.key).toBe("2026-09-05");
    expect(stats.days.length).toBeGreaterThanOrEqual(365);
  });

  it("deduplicates sessions and overlapping transcript timing while retaining resumed recording", () => {
    const stats = summarizeActivity(
      [
        record("a", "2026-09-04T09:00:00Z"),
        record("a", "2026-09-04T09:30:00Z"),
        record("a", "2026-09-04T12:00:00Z"),
        record("b", "2026-09-04T14:00:00Z"),
      ],
      now,
      "UTC",
    );
    expect(stats).toMatchObject({
      conversations: 2,
      activeDays: 1,
      hours: 3.5,
      nextMilestone: 10,
    });
    expect(stats.days.find((day) => day.key === "2026-09-04")?.count).toBe(2);
  });

  it("filters calendar days in the selected timezone while keeping all-time milestones", () => {
    const stats = summarizeActivity(
      [
        record("old", "2026-08-29T14:59:00Z"),
        record("edge", "2026-08-29T15:00:00Z"),
        record("today", "2026-09-04T23:00:00Z"),
      ],
      now,
      "Asia/Seoul",
      1,
      "7d",
    );
    expect(stats).toMatchObject({
      conversations: 2,
      activeDays: 2,
      totalConversations: 3,
    });
    expect(stats.days.find((day) => day.key === "2026-09-05")?.count).toBe(1);
  });

  it("keeps a weekly streak during an unfinished week and resets it after a missed week", () => {
    const records = [
      record("a", "2026-08-19T12:00:00Z"),
      record("b", "2026-08-26T12:00:00Z"),
    ];
    expect(summarizeActivity(records, now, "UTC", 1).streak).toBe(2);
    expect(
      summarizeActivity(records, new Date("2026-09-07T12:00:00Z"), "UTC", 1)
        .streak,
    ).toBe(0);
  });

  it("handles DST without dropping or duplicating heatmap days", () => {
    const stats = summarizeActivity(
      [
        record("a", "2026-03-08T06:30:00Z"),
        record("b", "2026-03-08T07:30:00Z"),
      ],
      now,
      "America/New_York",
    );
    expect(stats.activeDays).toBe(1);
    expect(new Set(stats.days.map((day) => day.key)).size).toBe(
      stats.days.length,
    );
    expect(stats.days.find((day) => day.key === "2026-03-08")?.count).toBe(2);
  });

  it("ignores invalid and future dates, falls back to creation time, and caps unfinished timing", () => {
    const stats = summarizeActivity(
      [
        record("future", "2027-01-01T00:00:00Z"),
        record("invalid", "invalid"),
        { ...record("legacy", "2026-09-04T00:00:00Z"), started_at_ms: 0 },
        record("current", "2026-09-05T11:30:00Z"),
      ],
      now,
      "UTC",
    );
    expect(stats).toMatchObject({ conversations: 2, hours: 1.5 });
  });

  it("advances milestone targets at thresholds and beyond the last badge", () => {
    for (const [total, nextMilestone] of [
      [10, 25],
      [1000, 2000],
      [2000, 3000],
    ]) {
      const stats = summarizeActivity(
        Array.from({ length: total }, (_, index) =>
          record(String(index), "2026-09-04T00:00:00Z"),
        ),
        now,
        "UTC",
      );
      expect(stats.nextMilestone).toBe(nextMilestone);
    }
  });
});
