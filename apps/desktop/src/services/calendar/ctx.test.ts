import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const pluginCalendar = vi.hoisted(() => ({
  listConnectionIds: vi.fn(),
  listCalendars: vi.fn(),
}));

const storage = vi.hoisted(() => ({
  applyCalendarInventory: vi.fn(),
  loadEnabledCalendars: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: {
    listConnectionIds: pluginCalendar.listConnectionIds,
    listCalendars: pluginCalendar.listCalendars,
  },
}));

vi.mock("./storage", () => storage);

import { createCtx, getProviderConnections, syncCalendars } from "./ctx";

describe("calendar sync context", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    storage.applyCalendarInventory.mockResolvedValue(undefined);
    storage.loadEnabledCalendars.mockResolvedValue([
      {
        id: "cal-1",
        tracking_id_calendar: "primary",
        name: "Work",
        enabled: true,
        provider: "google",
        source: "work@example.com",
        color: "#4285f4",
        connection_id: "conn-work",
        created_at: "2026-05-01T00:00:00.000Z",
        deleted_at: null,
      },
    ]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("limits the default event range to six days ago through tomorrow", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 4, 29, 13, 30));

    const ctx = await createCtx("google", "conn-work");

    expect(ctx.from).toEqual(new Date(2026, 4, 23, 0, 0, 0, 0));
    expect(ctx.to).toEqual(new Date(2026, 4, 31, 0, 0, 0, 0));
    expect(ctx.calendarTrackingIdToId).toEqual(new Map([["primary", "cal-1"]]));
  });

  test("uses an explicit event range", async () => {
    const range = {
      from: new Date("2026-06-01T00:00:00.000Z"),
      to: new Date("2026-07-01T00:00:00.000Z"),
    };

    const ctx = await createCtx("google", "conn-work", range);

    expect(ctx.from).toBe(range.from);
    expect(ctx.to).toBe(range.to);
  });

  test("surfaces connection discovery errors", async () => {
    pluginCalendar.listConnectionIds.mockResolvedValue({
      status: "error",
      error: "auth session could not be parsed",
    });

    await expect(getProviderConnections()).rejects.toThrow(
      "Failed to discover calendar connections: auth session could not be parsed",
    );
  });

  test("keeps overlapping calendar ids isolated by connection", async () => {
    pluginCalendar.listCalendars.mockImplementation(
      async (_provider: string, connectionId: string) => ({
        status: "success",
        data: [
          {
            id: "primary",
            title: connectionId === "conn-work" ? "Work" : "Personal",
          },
        ],
      }),
    );

    await syncCalendars([
      {
        provider: "google",
        connection_ids: ["conn-work", "conn-personal"],
      },
    ]);

    expect(storage.applyCalendarInventory).toHaveBeenCalledWith({
      provider: "google",
      requestedConnectionIds: ["conn-work", "conn-personal"],
      successfulConnections: [
        {
          connectionId: "conn-work",
          calendars: [{ id: "primary", title: "Work" }],
        },
        {
          connectionId: "conn-personal",
          calendars: [{ id: "primary", title: "Personal" }],
        },
      ],
    });
  });

  test("does not treat a failed calendar listing as an empty listing", async () => {
    pluginCalendar.listCalendars.mockImplementation(
      async (_provider: string, connectionId: string) =>
        connectionId === "conn-failed"
          ? { status: "error", error: "offline" }
          : { status: "success", data: [] },
    );

    await syncCalendars([
      {
        provider: "google",
        connection_ids: ["conn-ok", "conn-failed"],
      },
    ]);

    expect(storage.applyCalendarInventory).toHaveBeenCalledWith({
      provider: "google",
      requestedConnectionIds: ["conn-ok", "conn-failed"],
      successfulConnections: [{ connectionId: "conn-ok", calendars: [] }],
    });
  });

  test("does not write calendar inventory after cancellation", async () => {
    const abortController = new AbortController();
    pluginCalendar.listCalendars.mockImplementation(async () => {
      abortController.abort();
      return { status: "success", data: [] };
    });

    await syncCalendars(
      [{ provider: "google", connection_ids: ["conn-work"] }],
      abortController.signal,
    );

    expect(storage.applyCalendarInventory).not.toHaveBeenCalled();
  });
});
