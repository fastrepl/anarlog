import { describe, expect, test } from "vitest";

import {
  SCHEDULED_AUTO_START_GRACE_MS,
  type ScheduledMeetingRow,
  selectDueMeeting,
} from "./scheduled-auto-start";

const NOW = new Date("2026-05-15T12:00:00.000Z").getTime();

function meeting(
  id: string,
  offsetMs: number,
  overrides: Partial<ScheduledMeetingRow> = {},
): ScheduledMeetingRow {
  return {
    id,
    started_at: new Date(NOW + offsetMs).toISOString(),
    meeting_link: `https://zoom.us/j/${id}`,
    tracking_id_event: `tracking-${id}`,
    recurrence_series_id: "",
    ...overrides,
  };
}

function select(rows: ScheduledMeetingRow[], firedEventIds: string[] = []) {
  return selectDueMeeting({
    rows,
    nowMs: NOW,
    firedEventIds: new Set(firedEventIds),
  });
}

describe("selectDueMeeting", () => {
  test("selects a meeting whose start time has just arrived", () => {
    expect(select([meeting("a", 0)])?.id).toBe("a");
  });

  test("ignores meetings that have not started yet", () => {
    expect(select([meeting("a", 30_000)])).toBeNull();
  });

  test("selects a meeting that started within the grace window", () => {
    expect(select([meeting("a", -SCHEDULED_AUTO_START_GRACE_MS + 1)])?.id).toBe(
      "a",
    );
  });

  test("ignores meetings that started before the grace window", () => {
    expect(
      select([meeting("a", -SCHEDULED_AUTO_START_GRACE_MS - 1)]),
    ).toBeNull();
  });

  test("ignores meetings that already fired", () => {
    expect(select([meeting("a", 0)], ["a"])).toBeNull();
  });

  test("prefers the most recently started meeting when several are due", () => {
    const rows = [
      meeting("earlier", -4 * 60_000),
      meeting("latest", -30_000),
      meeting("middle", -2 * 60_000),
    ];

    expect(select(rows)?.id).toBe("latest");
  });

  test("skips rows with an unparseable start time", () => {
    const rows = [
      meeting("broken", 0, { started_at: "not-a-date" }),
      meeting("good", -60_000),
    ];

    expect(select(rows)?.id).toBe("good");
  });

  test("returns null when nothing is due", () => {
    expect(select([])).toBeNull();
  });
});
