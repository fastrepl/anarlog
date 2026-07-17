import { beforeEach, describe, expect, test, vi } from "vitest";

const ctxMocks = vi.hoisted(() => ({
  createCtx: vi.fn(),
  getProviderConnections: vi.fn(),
  syncCalendars: vi.fn(),
}));

const fetchMocks = vi.hoisted(() => ({
  fetchExistingEvents: vi.fn(),
  fetchIncomingEvents: vi.fn(),
}));

const processMocks = vi.hoisted(() => ({
  syncEvents: vi.fn(),
  syncSessionEmbeddedEvents: vi.fn(),
  syncSessionParticipants: vi.fn(),
}));

const storageMocks = vi.hoisted(() => ({
  applyConnectionSync: vi.fn(),
  loadParticipantSyncSnapshot: vi.fn(),
  loadSessionsForTrackingIds: vi.fn(),
}));

vi.mock("./ctx", () => ctxMocks);

vi.mock("./fetch", () => ({
  CalendarFetchError: class CalendarFetchError extends Error {},
  fetchExistingEvents: fetchMocks.fetchExistingEvents,
  fetchIncomingEvents: fetchMocks.fetchIncomingEvents,
}));

vi.mock("./process", () => processMocks);
vi.mock("./storage", () => storageMocks);
vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (
    _key: string,
    write: () => Promise<unknown>,
  ): Promise<unknown> => write(),
}));

import { syncCalendarEventsForRange } from ".";

const ctx = {
  provider: "google" as const,
  connectionId: "conn-1",
  from: new Date("2026-06-01T00:00:00.000Z"),
  to: new Date("2026-06-08T00:00:00.000Z"),
  calendarIds: new Set(["cal-1"]),
  calendarTrackingIdToId: new Map([["primary", "cal-1"]]),
};

describe("syncCalendarEventsForRange", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    ctxMocks.getProviderConnections.mockResolvedValue([
      { provider: "google", connection_ids: ["conn-1"] },
    ]);
    ctxMocks.syncCalendars.mockResolvedValue(undefined);
    ctxMocks.createCtx.mockResolvedValue(ctx);
    fetchMocks.fetchExistingEvents.mockResolvedValue([]);
    fetchMocks.fetchIncomingEvents.mockResolvedValue({
      events: [],
      participants: new Map(),
    });
    processMocks.syncEvents.mockReturnValue({
      toAdd: [],
      toDelete: [],
      toUpdate: [],
    });
    processMocks.syncSessionEmbeddedEvents.mockReturnValue([]);
    processMocks.syncSessionParticipants.mockReturnValue({
      humansToCreate: [],
      toAdd: [],
      toDelete: [],
    });
    storageMocks.loadSessionsForTrackingIds.mockResolvedValue([]);
    storageMocks.loadParticipantSyncSnapshot.mockResolvedValue({
      sessions: [],
      humans: [],
      mappings: [],
    });
    storageMocks.applyConnectionSync.mockResolvedValue(undefined);
  });

  test("does not start a range sync when already aborted", async () => {
    const abortController = new AbortController();
    abortController.abort();

    await syncCalendarEventsForRange(
      {
        from: new Date("2026-06-01T00:00:00.000Z"),
        to: new Date("2026-06-08T00:00:00.000Z"),
      },
      { signal: abortController.signal },
    );

    expect(ctxMocks.getProviderConnections).not.toHaveBeenCalled();
    expect(fetchMocks.fetchIncomingEvents).not.toHaveBeenCalled();
  });

  test("does not write fetched events after aborting a range sync", async () => {
    const abortController = new AbortController();
    fetchMocks.fetchIncomingEvents.mockImplementation(async () => {
      abortController.abort();
      return { events: [], participants: new Map() };
    });

    await syncCalendarEventsForRange(
      {
        from: new Date("2026-06-01T00:00:00.000Z"),
        to: new Date("2026-06-08T00:00:00.000Z"),
      },
      { signal: abortController.signal },
    );

    expect(fetchMocks.fetchIncomingEvents).toHaveBeenCalledTimes(1);
    expect(fetchMocks.fetchExistingEvents).not.toHaveBeenCalled();
    expect(storageMocks.applyConnectionSync).not.toHaveBeenCalled();
  });

  test("commits one connection snapshot after all diffs are built", async () => {
    const incoming = [
      {
        tracking_id_event: "event-1",
        tracking_id_calendar: "primary",
        has_recurrence_rules: false,
        is_all_day: false,
      },
    ];
    const incomingParticipants = new Map([["event-1", []]]);
    const events = { toAdd: [], toDelete: [], toUpdate: [] };
    const sessionUpdates = [{ sessionId: "session-1" }];
    const participants = { humansToCreate: [], toAdd: [], toDelete: [] };
    fetchMocks.fetchIncomingEvents.mockResolvedValue({
      events: incoming,
      participants: incomingParticipants,
    });
    processMocks.syncEvents.mockReturnValue(events);
    processMocks.syncSessionEmbeddedEvents.mockReturnValue(sessionUpdates);
    processMocks.syncSessionParticipants.mockReturnValue(participants);

    await syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });

    expect(fetchMocks.fetchExistingEvents).toHaveBeenCalledWith(ctx, incoming);
    expect(storageMocks.loadSessionsForTrackingIds).toHaveBeenCalledWith([
      "event-1",
    ]);
    expect(storageMocks.applyConnectionSync).toHaveBeenCalledWith({
      ctx,
      events,
      sessionUpdates,
      participants,
    });
  });
});
