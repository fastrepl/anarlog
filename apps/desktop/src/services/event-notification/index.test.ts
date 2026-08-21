import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  getIgnoredEventSets: vi.fn(() =>
    Promise.resolve({ ignoredIds: new Set(), ignoredSeriesIds: new Set() }),
  ),
  showNotification: vi.fn(),
}));

vi.mock("@anlg/plugin-notification", () => ({
  commands: { showNotification: mocks.showNotification },
}));

vi.mock("~/calendar/ignored-events", () => ({
  getIgnoredEventSets: mocks.getIgnoredEventSets,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

import { buildPreMeetingNotificationMessage, checkEventNotifications } from ".";

describe("checkEventNotifications", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.execute.mockReset();
    mocks.execute.mockResolvedValue([]);
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-05-15T12:00:00.000Z").getTime(),
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  test("scheduled meeting notifications use canonical SQLite events", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "event-1",
        started_at: "2026-05-15T12:02:00.000Z",
        tracking_id_event: "tracking-1",
        recurrence_series_id: "series-1",
        title: "Design Review",
        is_all_day: 0,
        location: "Studio",
        description: "",
        participants_json: JSON.stringify([
          { name: "Ada", email: "ada@example.com" },
        ]),
      },
    ]);
    mocks.execute.mockResolvedValueOnce([
      {
        body: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "paragraph",
              content: [
                { type: "text", text: "Ada will share the prototype." },
              ],
            },
          ],
        }),
      },
    ]);

    await checkEventNotifications(true, new Map());

    expect(mocks.showNotification).toHaveBeenCalledWith(
      expect.objectContaining({
        source: { type: "calendar_event", event_id: "event-1" },
        action_label: "Open Anarlog",
        participants: null,
        event_details: null,
        options: null,
        footer: null,
        message:
          "Starting in 2 minutes · Last time: Ada will share the prototype.",
        start_time: null,
        timeout: { secs: 420, nanos: 0 },
      }),
    );
    expect(mocks.execute.mock.calls[0]?.[0]).toContain("is_all_day = 0");
    expect(mocks.execute.mock.calls[1]?.[1]).toEqual([
      "series-1",
      "2026-05-15T12:02:00.000Z",
    ]);
  });

  test("does not query or notify when event notifications are disabled", async () => {
    await checkEventNotifications(false, new Map());

    expect(mocks.execute).not.toHaveBeenCalled();
    expect(mocks.showNotification).not.toHaveBeenCalled();
  });

  test("skips ignored tracking ids", async () => {
    mocks.getIgnoredEventSets.mockResolvedValueOnce({
      ignoredIds: new Set(["tracking-1"]),
      ignoredSeriesIds: new Set<string>(),
    });
    mocks.execute.mockResolvedValueOnce([
      {
        id: "event-1",
        started_at: "2026-05-15T12:02:00.000Z",
        tracking_id_event: "tracking-1",
        recurrence_series_id: "",
        title: "Design Review",
        is_all_day: 0,
        location: "",
        description: "",
        participants_json: "[]",
      },
    ]);

    await checkEventNotifications(true, new Map());

    expect(mocks.showNotification).not.toHaveBeenCalled();
  });

  test("skips all-day events even if the query returns one", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "event-1",
        started_at: "2026-05-15T12:02:00.000Z",
        tracking_id_event: "tracking-1",
        recurrence_series_id: "",
        title: "Company holiday",
        is_all_day: 1,
        location: "",
        description: "",
        participants_json: "[]",
      },
    ]);

    await checkEventNotifications(true, new Map());

    expect(mocks.showNotification).not.toHaveBeenCalled();
  });
});

describe("buildPreMeetingNotificationMessage", () => {
  test("falls back through calendar context without requiring history", () => {
    expect(
      buildPreMeetingNotificationMessage({
        minutesUntil: 5,
        participantNames: ["Ada", "Lin"],
      }),
    ).toBe("Starting in 5 minutes · With Ada, Lin");

    expect(
      buildPreMeetingNotificationMessage({
        minutesUntil: 1,
        location: "Conference room A",
      }),
    ).toBe("Starting in 1 minute · Conference room A");

    expect(buildPreMeetingNotificationMessage({ minutesUntil: 3 })).toBe(
      "Starting in 3 minutes",
    );
  });
});
