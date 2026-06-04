import { createMergeableStore, createQueries } from "tinybase/with-schemas";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { SCHEMA } from "@hypr/store";

const pluginCalendar = vi.hoisted(() => ({
  listCalendars: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: {
    listCalendars: pluginCalendar.listCalendars,
  },
}));

import { createCtx, syncCalendars } from "./ctx";

import { QUERIES } from "~/store/tinybase/store/main";

function createStore() {
  const store = createMergeableStore()
    .setTablesSchema(SCHEMA.table)
    .setValuesSchema(SCHEMA.value);

  store.setValue("user_id", "user-1");

  return store;
}

function getCalendarsByConnection(
  store: ReturnType<typeof createStore>,
  provider: string,
) {
  return store
    .getRowIds("calendars")
    .map((rowId) => ({ id: rowId, ...store.getRow("calendars", rowId) }))
    .filter((calendar) => calendar.provider === provider);
}

describe("syncCalendars", () => {
  beforeEach(() => {
    pluginCalendar.listCalendars.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("limits default event sync range to six days ago through tomorrow", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 4, 29, 13, 30));

    const store = createStore();
    store.setRow("calendars", "cal-1", {
      user_id: "user-1",
      created_at: "2026-05-01T00:00:00.000Z",
      tracking_id_calendar: "primary",
      name: "Work",
      enabled: true,
      provider: "google",
      source: "work@example.com",
      color: "#4285f4",
      connection_id: "conn-work",
    });
    const queries = createQueries(store).setQueryDefinition(
      QUERIES.enabledCalendars,
      "calendars",
      ({ select, where }) => {
        select("provider");
        where("enabled", true);
      },
    );

    const ctx = createCtx(store, queries, "google", "conn-work");

    expect(ctx?.from).toEqual(new Date(2026, 4, 23, 0, 0, 0, 0));
    expect(ctx?.to).toEqual(new Date(2026, 4, 31, 0, 0, 0, 0));
  });

  test("uses an explicit event sync range when provided", () => {
    const store = createStore();
    store.setRow("calendars", "cal-1", {
      user_id: "user-1",
      created_at: "2026-05-01T00:00:00.000Z",
      tracking_id_calendar: "primary",
      name: "Work",
      enabled: true,
      provider: "google",
      source: "work@example.com",
      color: "#4285f4",
      connection_id: "conn-work",
    });
    const queries = createQueries(store).setQueryDefinition(
      QUERIES.enabledCalendars,
      "calendars",
      ({ select, where }) => {
        select("provider");
        where("enabled", true);
      },
    );

    const range = {
      from: new Date("2026-06-01T00:00:00.000Z"),
      to: new Date("2026-07-01T00:00:00.000Z"),
    };
    const ctx = createCtx(store, queries, "google", "conn-work", range);

    expect(ctx?.from).toBe(range.from);
    expect(ctx?.to).toBe(range.to);
  });

  test("keeps Google calendars isolated per connection when ids overlap", async () => {
    const store = createStore();

    store.setRow("calendars", "john-row", {
      user_id: "user-1",
      created_at: "2026-03-25T00:00:00.000Z",
      tracking_id_calendar: "primary",
      name: "John (Anarlog)",
      enabled: true,
      provider: "google",
      source: "john@char.com",
      color: "#4285f4",
      connection_id: "conn-john",
    });

    pluginCalendar.listCalendars.mockImplementation(
      async (_provider: string, connectionId: string) => {
        if (connectionId === "conn-john") {
          return {
            status: "success",
            data: [
              {
                id: "primary",
                title: "John (Anarlog)",
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

    await syncCalendars(store, [
      {
        provider: "google",
        connection_ids: ["conn-john", "conn-gmail"],
      },
    ]);

    const calendars = getCalendarsByConnection(store, "google");

    expect(calendars).toHaveLength(2);
    expect(
      calendars.find((calendar) => calendar.connection_id === "conn-john"),
    ).toMatchObject({
      tracking_id_calendar: "primary",
      name: "John (Anarlog)",
      enabled: true,
      source: "john@char.com",
    });
    expect(
      calendars.find((calendar) => calendar.connection_id === "conn-gmail"),
    ).toMatchObject({
      tracking_id_calendar: "primary",
      name: "Personal",
      enabled: false,
      source: "jeeheontransformers@gmail.com",
    });
  });

  test("removes calendars for disconnected accounts even when ids overlap", async () => {
    const store = createStore();

    store.setRow("calendars", "john-row", {
      user_id: "user-1",
      created_at: "2026-03-25T00:00:00.000Z",
      tracking_id_calendar: "primary",
      name: "John (Anarlog)",
      enabled: true,
      provider: "google",
      source: "john@char.com",
      color: "#4285f4",
      connection_id: "conn-john",
    });
    store.setRow("calendars", "gmail-row", {
      user_id: "user-1",
      created_at: "2026-03-25T00:00:00.000Z",
      tracking_id_calendar: "primary",
      name: "Personal",
      enabled: false,
      provider: "google",
      source: "jeeheontransformers@gmail.com",
      color: "#a142f4",
      connection_id: "conn-gmail",
    });

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

    await syncCalendars(store, [
      {
        provider: "google",
        connection_ids: ["conn-gmail"],
      },
    ]);

    const calendars = getCalendarsByConnection(store, "google");

    expect(calendars).toHaveLength(1);
    expect(calendars[0]).toMatchObject({
      connection_id: "conn-gmail",
      name: "Personal",
    });
  });
});
