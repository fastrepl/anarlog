import { describe, expect, test } from "vitest";

import {
  getScheduledAutoStartAction,
  hasPendingAutoStart,
  SCHEDULED_AUTO_START_GRACE_MS,
  type ScheduledMeetingRow,
  selectDueMeetings,
} from "./scheduled-auto-start";

import type { Tab } from "~/store/zustand/tabs";

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
  return selectDueMeetings({
    rows,
    nowMs: NOW,
    firedEventIds: new Set(firedEventIds),
  }).map((row) => row.id);
}

describe("selectDueMeetings", () => {
  test("selects a meeting whose start time has just arrived", () => {
    expect(select([meeting("a", 0)])).toEqual(["a"]);
  });

  test("ignores meetings that have not started yet", () => {
    expect(select([meeting("a", 30_000)])).toEqual([]);
  });

  test("selects a meeting that started within the grace window", () => {
    expect(select([meeting("a", -SCHEDULED_AUTO_START_GRACE_MS + 1)])).toEqual([
      "a",
    ]);
  });

  test("ignores meetings that started before the grace window", () => {
    expect(select([meeting("a", -SCHEDULED_AUTO_START_GRACE_MS - 1)])).toEqual(
      [],
    );
  });

  test("ignores meetings that already fired", () => {
    expect(select([meeting("a", 0)], ["a"])).toEqual([]);
  });

  test("orders overlapping meetings by most recent start", () => {
    const rows = [
      meeting("earlier", -4 * 60_000),
      meeting("latest", -30_000),
      meeting("middle", -2 * 60_000),
    ];

    expect(select(rows)).toEqual(["latest", "middle", "earlier"]);
  });

  test("still returns an overlapping meeting when the newest already fired", () => {
    const rows = [meeting("earlier", -60_000), meeting("latest", -30_000)];

    expect(select(rows, ["latest"])).toEqual(["earlier"]);
  });

  test("skips rows with an unparseable start time", () => {
    const rows = [
      meeting("broken", 0, { started_at: "not-a-date" }),
      meeting("good", -60_000),
    ];

    expect(select(rows)).toEqual(["good"]);
  });

  test("returns nothing when no meeting is due", () => {
    expect(select([])).toEqual([]);
  });
});

describe("hasPendingAutoStart", () => {
  const sessionTab = (
    id: string,
    autoStart: boolean | null,
  ): Extract<Tab, { type: "sessions" }> => ({
    type: "sessions",
    id,
    active: true,
    slotId: id,
    pinned: false,
    state: { view: null, autoStart },
  });

  test("blocks another scheduled start while a tab is still arming", () => {
    expect(
      hasPendingAutoStart([
        sessionTab("ready", null),
        sessionTab("arming", true),
      ]),
    ).toBe(true);
  });

  test("allows scheduling after every pending start clears", () => {
    expect(hasPendingAutoStart([sessionTab("ready", null)])).toBe(false);
  });

  test("does not let an inactive pending tab block scheduling", () => {
    expect(
      hasPendingAutoStart([{ ...sessionTab("inactive", true), active: false }]),
    ).toBe(false);
  });
});

describe("getScheduledAutoStartAction", () => {
  test("starts when no live session is active", () => {
    expect(getScheduledAutoStartAction("inactive")).toBe("start");
  });

  test("retries while a live session is finalizing", () => {
    expect(getScheduledAutoStartAction("finalizing")).toBe("retry");
  });

  test("skips when a live session is active", () => {
    expect(getScheduledAutoStartAction("active")).toBe("skip");
  });
});
