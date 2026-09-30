import { cleanup, render } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  getScheduledAutoStartAction,
  hasPendingAutoStart,
  SCHEDULED_AUTO_START_GRACE_MS,
  type ScheduledMeetingRow,
  ScheduledMeetingAutoStart,
  selectDueMeetings,
  startScheduledMeeting,
} from "./scheduled-auto-start";

import type { Tab } from "~/store/zustand/tabs";

const mocks = vi.hoisted(() => ({
  canStart: true,
  liveStatus: "inactive",
  getIgnoredEventSets: vi.fn(),
  getOrCreateSessionForEventId: vi.fn(),
  openNew: vi.fn(),
  openUrl: vi.fn(),
  executeMeeting: vi.fn(),
  subscribeMeetings: vi.fn(),
  subscribeListener: vi.fn(),
  subscribeTabs: vi.fn(),
  tabs: [] as Tab[],
}));

vi.mock("@anlg/plugin-windows", () => ({
  getCurrentWebviewWindowLabel: () => "main",
}));

vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: mocks.executeMeeting,
    subscribe: mocks.subscribeMeetings,
  },
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => ({
    auto_start_scheduled_meetings: true,
    auto_join_scheduled_meetings: true,
  }),
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("~/calendar/ignored-events", () => ({
  getIgnoredEventSets: mocks.getIgnoredEventSets,
}));

vi.mock("~/session/queries", () => ({
  getOrCreateSessionForEventId: mocks.getOrCreateSessionForEventId,
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => ({
      canStartLiveSession: () => mocks.canStart,
      live: { status: mocks.liveStatus },
    }),
    subscribe: mocks.subscribeListener,
  },
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: {
    getState: () => ({ openNew: mocks.openNew, tabs: mocks.tabs }),
    subscribe: mocks.subscribeTabs,
  },
}));

const NOW = new Date("2026-05-15T12:00:00.000Z").getTime();

function attendance(
  selfStatus:
    | "organizer"
    | "accepted"
    | "tentative"
    | "pending"
    | "declined"
    | "unknown" = "accepted",
  overrides: Record<string, unknown> = {},
) {
  return JSON.stringify({
    version: 1,
    self_status: selfStatus,
    roster_status: "complete",
    others: {
      accepted: 1,
      tentative: 0,
      pending: 0,
      declined: 0,
      unknown: 0,
    },
    observed_at: new Date(NOW).toISOString(),
    ...overrides,
  });
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

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
    attendance_json: attendance(),
    ...overrides,
  };
}

function currentMeeting(
  id: string,
  overrides: Partial<ScheduledMeetingRow> = {},
): ScheduledMeetingRow {
  return meeting(id, 0, {
    started_at: new Date().toISOString(),
    attendance_json: attendance("accepted", {
      observed_at: new Date().toISOString(),
    }),
    ...overrides,
  });
}

function select(rows: ScheduledMeetingRow[], firedEventIds: string[] = []) {
  return selectDueMeetings({
    rows,
    nowMs: NOW,
    firedEventIds: new Set(firedEventIds),
  }).map((row) => row.id);
}

describe("selectDueMeetings", () => {
  test.each([
    {
      name: "selects a meeting whose start time has just arrived",
      rows: [meeting("a", 0)],
      fired: [],
      expected: ["a"],
    },
    {
      name: "ignores meetings that have not started yet",
      rows: [meeting("a", 30_000)],
      fired: [],
      expected: [],
    },
    {
      name: "selects a meeting that started within the grace window",
      rows: [meeting("a", -SCHEDULED_AUTO_START_GRACE_MS + 1)],
      fired: [],
      expected: ["a"],
    },
    {
      name: "ignores meetings that started before the grace window",
      rows: [meeting("a", -SCHEDULED_AUTO_START_GRACE_MS - 1)],
      fired: [],
      expected: [],
    },
    {
      name: "ignores meetings that already fired",
      rows: [meeting("a", 0)],
      fired: ["a"],
      expected: [],
    },
    {
      name: "orders overlapping meetings by most recent start",
      rows: [
        meeting("earlier", -4 * 60_000),
        meeting("latest", -30_000),
        meeting("middle", -2 * 60_000),
      ],
      fired: [],
      expected: ["latest", "middle", "earlier"],
    },
    {
      name: "still returns an overlapping meeting when the newest already fired",
      rows: [meeting("earlier", -60_000), meeting("latest", -30_000)],
      fired: ["latest"],
      expected: ["earlier"],
    },
    {
      name: "skips rows with an unparseable start time",
      rows: [
        meeting("broken", 0, { started_at: "not-a-date" }),
        meeting("good", -60_000),
      ],
      fired: [],
      expected: ["good"],
    },
    {
      name: "treats timezone-naive Graph timestamps as UTC",
      rows: [
        meeting("naive", 0, { started_at: "2026-05-15T12:00:00.0000000" }),
      ],
      fired: [],
      expected: ["naive"],
    },
    {
      name: "returns nothing when no meeting is due",
      rows: [],
      fired: [],
      expected: [],
    },
  ])("$name", ({ rows, fired, expected }) => {
    expect(select(rows, fired)).toEqual(expected);
  });

  test.each(["tentative", "pending", "declined", "unknown"] as const)(
    "does not select a meeting when self attendance is %s",
    (selfStatus) => {
      expect(
        select([meeting("a", 0, { attendance_json: attendance(selfStatus) })]),
      ).toEqual([]);
    },
  );

  test("does not let a newer ineligible meeting hide an older eligible one", () => {
    expect(
      select([
        meeting("older", -60_000),
        meeting("newer", 0, {
          attendance_json: attendance("pending"),
        }),
      ]),
    ).toEqual(["older"]);
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

describe("startScheduledMeeting", () => {
  beforeEach(() => {
    mocks.canStart = true;
    mocks.liveStatus = "inactive";
    mocks.getIgnoredEventSets.mockReset().mockResolvedValue({
      ignoredIds: new Set<string>(),
      ignoredSeriesIds: new Set<string>(),
    });
    mocks.getOrCreateSessionForEventId
      .mockReset()
      .mockResolvedValue("session-a");
    mocks.executeMeeting
      .mockReset()
      .mockImplementation(async (_sql, params: string[]) => [
        currentMeeting(params[0] ?? "a"),
      ]);
    mocks.openNew.mockReset();
    mocks.openUrl.mockReset().mockResolvedValue({ status: "ok", data: null });
    mocks.subscribeMeetings.mockReset().mockResolvedValue(async () => {});
    mocks.subscribeListener.mockReset().mockReturnValue(() => {});
    mocks.subscribeTabs.mockReset().mockReturnValue(() => {});
    mocks.tabs = [];
  });

  test("opens the meeting link and arms the session when the meeting is due", async () => {
    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "started",
    );

    expect(mocks.openUrl).toHaveBeenCalledWith("https://zoom.us/j/a", null);
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "sessions",
      id: "session-a",
      state: { view: null, autoStart: true },
    });
  });

  test("only arms the session when auto-join is off", async () => {
    await expect(startScheduledMeeting(meeting("a", 0), false)).resolves.toBe(
      "started",
    );

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).toHaveBeenCalledTimes(1);
  });

  test("does nothing when the defensive read sees a decline", async () => {
    mocks.executeMeeting.mockResolvedValue([
      currentMeeting("a", {
        attendance_json: attendance("declined", {
          observed_at: new Date().toISOString(),
        }),
      }),
    ]);

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ineligible",
    );

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("re-checks attendance after session creation before automatic actions", async () => {
    mocks.executeMeeting
      .mockResolvedValueOnce([currentMeeting("a")])
      .mockResolvedValueOnce([
        currentMeeting("a", {
          attendance_json: attendance("declined", {
            observed_at: new Date().toISOString(),
          }),
        }),
      ]);

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ineligible",
    );

    expect(mocks.getOrCreateSessionForEventId).toHaveBeenCalledWith("a");
    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("does nothing when the meeting is rescheduled before starting", async () => {
    mocks.executeMeeting.mockResolvedValue([
      currentMeeting("a", {
        started_at: new Date(Date.now() + 60_000).toISOString(),
      }),
    ]);

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ineligible",
    );

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("does nothing when the event disappeared before starting", async () => {
    mocks.executeMeeting.mockResolvedValue([]);

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ineligible",
    );

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("uses the latest meeting link from the defensive read", async () => {
    mocks.executeMeeting.mockResolvedValue([
      currentMeeting("a", { meeting_link: "https://meet.example/latest" }),
    ]);

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "started",
    );

    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://meet.example/latest",
      null,
    );
  });

  test("does not open the link while the session cannot start yet", async () => {
    mocks.canStart = false;

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "blocked",
    );

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("skips ignored events entirely", async () => {
    mocks.getIgnoredEventSets.mockResolvedValue({
      ignoredIds: new Set(["tracking-a"]),
      ignoredSeriesIds: new Set<string>(),
    });

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ignored",
    );

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("re-checks ignored identities before automatic actions", async () => {
    mocks.getIgnoredEventSets
      .mockResolvedValueOnce({
        ignoredIds: new Set<string>(),
        ignoredSeriesIds: new Set<string>(),
      })
      .mockResolvedValueOnce({
        ignoredIds: new Set(["tracking-latest"]),
        ignoredSeriesIds: new Set<string>(),
      });
    mocks.executeMeeting.mockResolvedValue([
      currentMeeting("a", { tracking_id_event: "tracking-latest" }),
    ]);

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ignored",
    );

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("ignores the next meeting while the previous recording runs overtime", async () => {
    mocks.liveStatus = "active";

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ignored",
    );

    expect(mocks.getOrCreateSessionForEventId).not.toHaveBeenCalled();
    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("ignores a recording that becomes active while the calendar lookup is pending", async () => {
    mocks.getIgnoredEventSets.mockImplementation(async () => {
      mocks.liveStatus = "active";
      return { ignoredIds: new Set(), ignoredSeriesIds: new Set() };
    });

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ignored",
    );

    expect(mocks.getOrCreateSessionForEventId).not.toHaveBeenCalled();
    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("does not queue or join a meeting if recording starts during session creation", async () => {
    mocks.getOrCreateSessionForEventId.mockImplementation(async () => {
      mocks.liveStatus = "active";
      mocks.canStart = false;
      return "session-a";
    });

    await expect(startScheduledMeeting(meeting("a", 0), true)).resolves.toBe(
      "ignored",
    );

    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("an overlapping meeting stays skipped after a pending tab and the active recording clear", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    mocks.liveStatus = "active";
    mocks.tabs = [
      {
        type: "sessions",
        id: "pending",
        slotId: "pending",
        active: true,
        pinned: false,
        state: { view: null, autoStart: true },
      },
    ];
    render(createElement(ScheduledMeetingAutoStart));
    mocks.subscribeMeetings.mock.calls[0][2].onData([meeting("a", 0)]);

    mocks.liveStatus = "inactive";
    mocks.tabs = [];
    mocks.subscribeTabs.mock.calls[0][0]();
    await vi.advanceTimersByTimeAsync(15_000);

    expect(mocks.getOrCreateSessionForEventId).not.toHaveBeenCalled();
    expect(mocks.openUrl).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  test("starts when a due unanswered meeting becomes accepted", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    render(createElement(ScheduledMeetingAutoStart));
    const onData = mocks.subscribeMeetings.mock.calls[0][2].onData;

    onData([meeting("a", 0, { attendance_json: attendance("pending") })]);
    expect(mocks.getOrCreateSessionForEventId).not.toHaveBeenCalled();

    onData([meeting("a", 0)]);
    await vi.advanceTimersByTimeAsync(0);

    expect(mocks.getOrCreateSessionForEventId).toHaveBeenCalledWith("a");
    expect(mocks.openUrl).toHaveBeenCalledWith("https://zoom.us/j/a", null);
    expect(mocks.openNew).toHaveBeenCalledTimes(1);
  });

  test("tries an older overlapping meeting when the newest becomes ineligible", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    mocks.executeMeeting.mockImplementation(async (_sql, params: string[]) => [
      params[0] === "newer"
        ? currentMeeting("newer", {
            attendance_json: attendance("declined", {
              observed_at: new Date().toISOString(),
            }),
          })
        : currentMeeting("older"),
    ]);
    render(createElement(ScheduledMeetingAutoStart));

    mocks.subscribeMeetings.mock.calls[0][2].onData([
      meeting("older", -60_000),
      meeting("newer", 0),
    ]);
    await vi.advanceTimersByTimeAsync(1);

    expect(mocks.getOrCreateSessionForEventId).toHaveBeenCalledWith("older");
    expect(mocks.openUrl).toHaveBeenCalledWith("https://zoom.us/j/older", null);
    expect(mocks.openNew).toHaveBeenCalledTimes(1);
  });

  test("retries an event when calendar data changes during an ineligible read", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    let resolveStaleRead: (rows: ScheduledMeetingRow[]) => void = () => {};
    mocks.executeMeeting
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveStaleRead = resolve;
        }),
      )
      .mockImplementation(async (_sql, params: string[]) => [
        currentMeeting(params[0] ?? "a"),
      ]);
    render(createElement(ScheduledMeetingAutoStart));
    const onData = mocks.subscribeMeetings.mock.calls[0][2].onData;

    onData([meeting("a", 0)]);
    onData([meeting("a", 0, { meeting_link: "https://meet.example/fresh" })]);
    resolveStaleRead([
      currentMeeting("a", {
        attendance_json: attendance("declined", {
          observed_at: new Date().toISOString(),
        }),
      }),
    ]);
    await vi.advanceTimersByTimeAsync(1);

    expect(mocks.openUrl).toHaveBeenCalledWith("https://zoom.us/j/a", null);
    expect(mocks.openNew).toHaveBeenCalledTimes(1);
  });
});

describe("getScheduledAutoStartAction", () => {
  test.each([
    { status: "inactive" as const, expected: "start" },
    { status: "finalizing" as const, expected: "retry" },
    { status: "active" as const, expected: "skip" },
  ])("returns $expected for a $status live session", ({ status, expected }) => {
    expect(getScheduledAutoStartAction(status)).toBe(expected);
  });
});
