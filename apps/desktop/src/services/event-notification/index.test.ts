import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  getIgnoredEventSets: vi.fn(() =>
    Promise.resolve({ ignoredIds: new Set(), ignoredSeriesIds: new Set() }),
  ),
  showNotification: vi.fn(),
}));

vi.mock("@hypr/plugin-notification", () => ({
  commands: { showNotification: mocks.showNotification },
}));

vi.mock("~/calendar/ignored-events", () => ({
  getIgnoredEventSets: mocks.getIgnoredEventSets,
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
}));

import { checkEventNotifications } from ".";

describe("checkEventNotifications", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
        recurrence_series_id: "",
        title: "Design Review",
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
      }),
    );
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
      },
    ]);

    await checkEventNotifications(true, new Map());

    expect(mocks.showNotification).not.toHaveBeenCalled();
  });
});
