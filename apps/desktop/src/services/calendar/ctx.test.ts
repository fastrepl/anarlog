import { beforeEach, describe, expect, test, vi } from "vitest";

const pluginCalendar = vi.hoisted(() => ({
  listCalendars: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: {
    listCalendars: pluginCalendar.listCalendars,
  },
}));

const calendarQueries = vi.hoisted(() => ({
  getAllCalendars: vi.fn(),
  getEnabledCalendars: vi.fn(),
  insertCalendar: vi.fn(),
  updateCalendar: vi.fn(),
  deleteCalendar: vi.fn(),
  deleteEventsByCalendarId: vi.fn(),
}));

vi.mock("~/calendar/queries", () => calendarQueries);

vi.mock("~/calendar/utils", async (importOriginal) => {
  const original = await importOriginal<typeof import("~/calendar/utils")>();
  return {
    getCalendarTrackingKey: original.getCalendarTrackingKey,
    findCalendarByTrackingId: vi.fn(
      async ({ provider, connectionId, trackingId }) => {
        const calendars = await calendarQueries.getAllCalendars();
        for (const cal of calendars) {
          if (
            cal.provider === provider &&
            cal.connectionId === connectionId &&
            cal.trackingIdCalendar === trackingId
          ) {
            return cal.id;
          }
        }
        return null;
      },
    ),
  };
});

import { syncCalendars } from "./ctx";

type CalRecord = {
  id: string;
  trackingIdCalendar: string;
  name: string;
  enabled: boolean;
  provider: string;
  source: string;
  color: string;
  connectionId: string;
  createdAt: string;
};

describe("syncCalendars", () => {
  let calendarsDb: CalRecord[];

  beforeEach(() => {
    pluginCalendar.listCalendars.mockReset();
    calendarsDb = [];

    calendarQueries.getAllCalendars.mockImplementation(async () => [
      ...calendarsDb,
    ]);
    calendarQueries.getEnabledCalendars.mockImplementation(async () =>
      calendarsDb.filter((c) => c.enabled),
    );
    calendarQueries.insertCalendar.mockImplementation(
      async (cal: CalRecord) => {
        calendarsDb.push({
          ...cal,
          createdAt: cal.createdAt ?? new Date().toISOString(),
        });
      },
    );
    calendarQueries.updateCalendar.mockImplementation(
      async (id: string, fields: Partial<CalRecord>) => {
        const idx = calendarsDb.findIndex((c) => c.id === id);
        if (idx >= 0) {
          calendarsDb[idx] = { ...calendarsDb[idx], ...fields };
        }
      },
    );
    calendarQueries.deleteCalendar.mockImplementation(async (id: string) => {
      calendarsDb = calendarsDb.filter((c) => c.id !== id);
    });
    calendarQueries.deleteEventsByCalendarId.mockResolvedValue(undefined);
  });

  test("keeps Google calendars isolated per connection when ids overlap", async () => {
    calendarsDb.push({
      id: "john-row",
      trackingIdCalendar: "primary",
      name: "John (Char)",
      enabled: true,
      provider: "google",
      source: "john@char.com",
      color: "#4285f4",
      connectionId: "conn-john",
      createdAt: "2026-03-25T00:00:00.000Z",
    });

    pluginCalendar.listCalendars.mockImplementation(
      async (_provider: string, connectionId: string) => {
        if (connectionId === "conn-john") {
          return {
            status: "success",
            data: [
              {
                id: "primary",
                title: "John (Char)",
                source: "john@char.com",
                color: "#4285f4",
              },
            ],
          };
        }

        if (connectionId === "conn-gmail") {
          return {
            status: "success",
            data: [
              {
                id: "primary",
                title: "Personal",
                source: "jeeheontransformers@gmail.com",
                color: "#a142f4",
              },
            ],
          };
        }

        return { status: "error" };
      },
    );

    await syncCalendars([
      {
        provider: "google",
        connection_ids: ["conn-john", "conn-gmail"],
      },
    ]);

    const googleCalendars = calendarsDb.filter((c) => c.provider === "google");

    expect(googleCalendars).toHaveLength(2);
    expect(
      googleCalendars.find((c) => c.connectionId === "conn-john"),
    ).toMatchObject({
      trackingIdCalendar: "primary",
      name: "John (Char)",
      enabled: true,
      source: "john@char.com",
    });
    expect(
      googleCalendars.find((c) => c.connectionId === "conn-gmail"),
    ).toMatchObject({
      trackingIdCalendar: "primary",
      name: "Personal",
      enabled: false,
      source: "jeeheontransformers@gmail.com",
    });
  });

  test("removes calendars for disconnected accounts even when ids overlap", async () => {
    calendarsDb.push(
      {
        id: "john-row",
        trackingIdCalendar: "primary",
        name: "John (Char)",
        enabled: true,
        provider: "google",
        source: "john@char.com",
        color: "#4285f4",
        connectionId: "conn-john",
        createdAt: "2026-03-25T00:00:00.000Z",
      },
      {
        id: "gmail-row",
        trackingIdCalendar: "primary",
        name: "Personal",
        enabled: false,
        provider: "google",
        source: "jeeheontransformers@gmail.com",
        color: "#a142f4",
        connectionId: "conn-gmail",
        createdAt: "2026-03-25T00:00:00.000Z",
      },
    );

    pluginCalendar.listCalendars.mockResolvedValue({
      status: "success",
      data: [
        {
          id: "primary",
          title: "Personal",
          source: "jeeheontransformers@gmail.com",
          color: "#a142f4",
        },
      ],
    });

    await syncCalendars([
      {
        provider: "google",
        connection_ids: ["conn-gmail"],
      },
    ]);

    const googleCalendars = calendarsDb.filter((c) => c.provider === "google");

    expect(googleCalendars).toHaveLength(1);
    expect(googleCalendars[0]).toMatchObject({
      connectionId: "conn-gmail",
      name: "Personal",
    });
  });
});
