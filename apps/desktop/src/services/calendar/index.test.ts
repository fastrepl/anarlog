import { createMergeableStore, createQueries } from "tinybase/with-schemas";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { SCHEMA } from "@hypr/store";

const pluginCalendar = vi.hoisted(() => ({
  listCalendars: vi.fn(),
  listConnectionIds: vi.fn(),
}));

const fetchMocks = vi.hoisted(() => ({
  fetchExistingEvents: vi.fn(),
  fetchIncomingEvents: vi.fn(),
}));

const processMocks = vi.hoisted(() => ({
  executeForEventsSync: vi.fn(),
  executeForParticipantsSync: vi.fn(),
  syncEvents: vi.fn(),
  syncSessionEmbeddedEvents: vi.fn(),
  syncSessionParticipants: vi.fn(),
}));

vi.mock("@hypr/plugin-calendar", () => ({
  commands: {
    listCalendars: pluginCalendar.listCalendars,
    listConnectionIds: pluginCalendar.listConnectionIds,
  },
}));

vi.mock("./fetch", () => ({
  CalendarFetchError: class CalendarFetchError extends Error {},
  fetchExistingEvents: fetchMocks.fetchExistingEvents,
  fetchIncomingEvents: fetchMocks.fetchIncomingEvents,
}));

vi.mock("./process", () => ({
  executeForEventsSync: processMocks.executeForEventsSync,
  executeForParticipantsSync: processMocks.executeForParticipantsSync,
  syncEvents: processMocks.syncEvents,
  syncSessionEmbeddedEvents: processMocks.syncSessionEmbeddedEvents,
  syncSessionParticipants: processMocks.syncSessionParticipants,
}));

import { syncCalendarEventsForRange } from ".";

import { QUERIES } from "~/store/tinybase/store/main";

function createStoreAndQueries() {
  const store = createMergeableStore()
    .setTablesSchema(SCHEMA.table)
    .setValuesSchema(SCHEMA.value);

  store.setValue("user_id", "user-1");
  store.setRow("calendars", "cal-1", {
    user_id: "user-1",
    created_at: "2026-05-01T00:00:00.000Z",
    tracking_id_calendar: "primary",
    name: "Work",
    enabled: true,
    provider: "google",
    source: "work@example.com",
    color: "#4285f4",
    connection_id: "conn-1",
  });

  const queries = createQueries(store).setQueryDefinition(
    QUERIES.enabledCalendars,
    "calendars",
    ({ select, where }) => {
      select("provider");
      where("enabled", true);
    },
  );

  return { store, queries };
}

describe("syncCalendarEventsForRange", () => {
  beforeEach(() => {
    vi.resetAllMocks();

    pluginCalendar.listConnectionIds.mockResolvedValue({
      status: "success",
      data: [{ provider: "google", connection_ids: ["conn-1"] }],
    });
    pluginCalendar.listCalendars.mockResolvedValue({
      status: "success",
      data: [
        {
          id: "primary",
          title: "Work",
          source: "work@example.com",
          color: "#4285f4",
        },
      ],
    });
    fetchMocks.fetchExistingEvents.mockReturnValue([]);
    fetchMocks.fetchIncomingEvents.mockResolvedValue({
      events: [],
      participants: [],
    });
    processMocks.syncEvents.mockReturnValue({});
    processMocks.syncSessionParticipants.mockReturnValue({});
  });

  test("does not start a range sync when already aborted", async () => {
    const { store, queries } = createStoreAndQueries();
    const abortController = new AbortController();
    abortController.abort();

    await syncCalendarEventsForRange(
      store,
      queries,
      {
        from: new Date("2026-06-01T00:00:00.000Z"),
        to: new Date("2026-06-08T00:00:00.000Z"),
      },
      { signal: abortController.signal },
    );

    expect(pluginCalendar.listConnectionIds).not.toHaveBeenCalled();
    expect(fetchMocks.fetchIncomingEvents).not.toHaveBeenCalled();
  });

  test("does not write fetched events after aborting a range sync", async () => {
    const { store, queries } = createStoreAndQueries();
    const abortController = new AbortController();
    fetchMocks.fetchIncomingEvents.mockImplementation(async () => {
      abortController.abort();
      return { events: [], participants: [] };
    });

    await syncCalendarEventsForRange(
      store,
      queries,
      {
        from: new Date("2026-06-01T00:00:00.000Z"),
        to: new Date("2026-06-08T00:00:00.000Z"),
      },
      { signal: abortController.signal },
    );

    expect(fetchMocks.fetchIncomingEvents).toHaveBeenCalledTimes(1);
    expect(fetchMocks.fetchExistingEvents).not.toHaveBeenCalled();
    expect(processMocks.executeForEventsSync).not.toHaveBeenCalled();
    expect(processMocks.syncSessionEmbeddedEvents).not.toHaveBeenCalled();
    expect(processMocks.executeForParticipantsSync).not.toHaveBeenCalled();
  });
});
