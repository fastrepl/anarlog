import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

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
  tombstoneCalendarConnection: vi.fn(),
}));

const writeQueueMocks = vi.hoisted(() => ({
  enqueueDatabaseWrite: vi.fn(),
}));

vi.mock("./ctx", () => ctxMocks);

vi.mock("./fetch", () => ({
  CalendarFetchError: class CalendarFetchError extends Error {},
  fetchExistingEvents: fetchMocks.fetchExistingEvents,
  fetchIncomingEvents: fetchMocks.fetchIncomingEvents,
}));

vi.mock("./process", () => processMocks);
vi.mock("./storage", () => storageMocks);
vi.mock("~/db/write-queue", () => writeQueueMocks);

import {
  allowReconnectedCalendarConnections,
  CALENDAR_SYNC_TASK_ID,
  removeDisconnectedCalendarConnection,
  scheduleCalendarSync,
  syncCalendarEvents,
  syncCalendarEventsForRange,
} from ".";

import { createTaskScheduler } from "~/services/task-scheduler";

const ctx = {
  provider: "google" as const,
  connectionId: "conn-1",
  from: new Date("2026-06-01T00:00:00.000Z"),
  to: new Date("2026-06-08T00:00:00.000Z"),
  calendarIds: new Set(["cal-1"]),
  calendarTrackingIdToId: new Map([["primary", "cal-1"]]),
};

const managers: ReturnType<typeof createTaskScheduler>[] = [];

describe("syncCalendarEventsForRange", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    writeQueueMocks.enqueueDatabaseWrite.mockImplementation(
      (_key: string, write: () => Promise<unknown>) => write(),
    );
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
    storageMocks.tombstoneCalendarConnection.mockResolvedValue(undefined);
  });

  afterEach(() => {
    for (const manager of managers) {
      manager.stop();
    }
    managers.length = 0;
    vi.useRealTimers();
  });

  test("removes the exact disconnected calendar connection", async () => {
    await removeDisconnectedCalendarConnection(
      "google-calendar",
      "conn-personal",
    );

    expect(storageMocks.tombstoneCalendarConnection).toHaveBeenCalledWith(
      "google",
      "conn-personal",
    );
    expect(ctxMocks.getProviderConnections).not.toHaveBeenCalled();
  });

  test("allows recovery sync when disconnect persistence fails", async () => {
    storageMocks.tombstoneCalendarConnection.mockRejectedValueOnce(
      new Error("write failed"),
    );

    await expect(
      removeDisconnectedCalendarConnection("google-calendar", "conn-1"),
    ).rejects.toThrow("write failed");
    await syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });

    expect(ctxMocks.syncCalendars).toHaveBeenCalledWith(
      [{ provider: "google", connection_ids: ["conn-1"] }],
      undefined,
      expect.any(Function),
    );
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

  test("does not start a scheduled sync when already aborted", async () => {
    const abortController = new AbortController();
    abortController.abort();

    await syncCalendarEvents({ signal: abortController.signal });

    expect(ctxMocks.getProviderConnections).not.toHaveBeenCalled();
  });

  test("does not hold the database write queue while remote discovery is pending", async () => {
    let resolveConnections:
      | ((
          connections: Array<{ provider: string; connection_ids: string[] }>,
        ) => void)
      | undefined;
    ctxMocks.getProviderConnections.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveConnections = resolve;
      }),
    );

    const sync = syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });
    await vi.waitFor(() => {
      expect(ctxMocks.getProviderConnections).toHaveBeenCalledOnce();
    });

    expect(writeQueueMocks.enqueueDatabaseWrite).not.toHaveBeenCalled();

    resolveConnections?.([]);
    await sync;
  });

  test("serializes remote calendar syncs outside the database write queue", async () => {
    let resolveFirst:
      | ((
          connections: Array<{ provider: string; connection_ids: string[] }>,
        ) => void)
      | undefined;
    ctxMocks.getProviderConnections
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockResolvedValueOnce([]);

    const first = syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });
    const second = syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });
    await vi.waitFor(() => {
      expect(ctxMocks.getProviderConnections).toHaveBeenCalledOnce();
    });

    resolveFirst?.([]);
    await first;
    await second;

    expect(ctxMocks.getProviderConnections).toHaveBeenCalledTimes(2);
    expect(writeQueueMocks.enqueueDatabaseWrite).not.toHaveBeenCalled();
  });

  test("starts a queued sync with the latest connection generation", async () => {
    let resolveFirst:
      | ((
          connections: Array<{ provider: string; connection_ids: string[] }>,
        ) => void)
      | undefined;
    ctxMocks.getProviderConnections.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveFirst = resolve;
      }),
    );

    const first = syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });
    const second = syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });
    await vi.waitFor(() => {
      expect(ctxMocks.getProviderConnections).toHaveBeenCalledOnce();
    });

    await removeDisconnectedCalendarConnection(
      "google-calendar",
      "conn-removed",
    );
    resolveFirst?.([{ provider: "google", connection_ids: ["conn-1"] }]);
    await Promise.all([first, second]);

    expect(ctxMocks.getProviderConnections).toHaveBeenCalledTimes(2);
    expect(ctxMocks.syncCalendars).toHaveBeenCalledWith(
      [{ provider: "google", connection_ids: ["conn-1"] }],
      undefined,
      expect.any(Function),
    );
  });

  test("disconnect invalidates stale work and releases its guard after deletion is observed", async () => {
    let resolveConnections:
      | ((
          connections: Array<{ provider: string; connection_ids: string[] }>,
        ) => void)
      | undefined;
    ctxMocks.getProviderConnections.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveConnections = resolve;
      }),
    );

    const sync = syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });
    await vi.waitFor(() => {
      expect(ctxMocks.getProviderConnections).toHaveBeenCalledOnce();
    });

    await removeDisconnectedCalendarConnection("google-calendar", "conn-1");
    resolveConnections?.([{ provider: "google", connection_ids: ["conn-1"] }]);
    await sync;

    expect(storageMocks.tombstoneCalendarConnection).toHaveBeenCalledWith(
      "google",
      "conn-1",
    );
    expect(ctxMocks.syncCalendars).not.toHaveBeenCalled();
    expect(fetchMocks.fetchIncomingEvents).not.toHaveBeenCalled();

    ctxMocks.getProviderConnections.mockResolvedValueOnce([]);
    await syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });

    ctxMocks.syncCalendars.mockClear();
    await syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });
    expect(ctxMocks.syncCalendars).toHaveBeenCalledWith(
      [{ provider: "google", connection_ids: ["conn-1"] }],
      undefined,
      expect.any(Function),
    );
    expect(ctxMocks.createCtx).toHaveBeenCalledWith(
      "google",
      "conn-1",
      expect.anything(),
    );
  });

  test("ignores a stale connection without syncing provider inventory", async () => {
    await removeDisconnectedCalendarConnection("google-calendar", "conn-1");
    ctxMocks.syncCalendars.mockClear();

    await syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });

    expect(ctxMocks.syncCalendars).toHaveBeenCalledWith(
      [],
      undefined,
      expect.any(Function),
    );
    expect(ctxMocks.createCtx).not.toHaveBeenCalled();
    expect(fetchMocks.fetchIncomingEvents).not.toHaveBeenCalled();

    ctxMocks.getProviderConnections.mockResolvedValueOnce([]);
    await syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });
  });

  test("allows a calendar connection after the provider reconnects", async () => {
    await removeDisconnectedCalendarConnection("google-calendar", "conn-1");
    allowReconnectedCalendarConnections("google-calendar");
    ctxMocks.syncCalendars.mockClear();

    await syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });

    expect(ctxMocks.syncCalendars).toHaveBeenCalledWith(
      [{ provider: "google", connection_ids: ["conn-1"] }],
      undefined,
      expect.any(Function),
    );
    expect(ctxMocks.createCtx).toHaveBeenCalledWith(
      "google",
      "conn-1",
      expect.anything(),
    );
  });

  test("reconnect invalidates a sync that filtered the disconnected connection", async () => {
    await removeDisconnectedCalendarConnection("google-calendar", "conn-1");
    ctxMocks.syncCalendars.mockImplementationOnce(
      async (_connections, _signal, shouldStop) => {
        allowReconnectedCalendarConnections("google-calendar");
        expect(shouldStop()).toBe(true);
      },
    );

    await syncCalendarEventsForRange({ from: ctx.from, to: ctx.to });

    expect(ctxMocks.syncCalendars).toHaveBeenCalledWith(
      [],
      undefined,
      expect.any(Function),
    );
    expect(ctxMocks.createCtx).not.toHaveBeenCalled();
  });

  test("reuses an active scheduled calendar sync", () => {
    const manager = createTaskScheduler();
    managers.push(manager);
    manager.setTask(CALENDAR_SYNC_TASK_ID, async () => undefined);

    const firstTaskRunId = scheduleCalendarSync(manager);
    const secondTaskRunId = scheduleCalendarSync(manager);

    expect(firstTaskRunId).toBeDefined();
    expect(secondTaskRunId).toBe(firstTaskRunId);
  });

  test("reuses a running calendar sync", async () => {
    vi.useFakeTimers();
    const manager = createTaskScheduler();
    managers.push(manager);
    manager.setTask(
      CALENDAR_SYNC_TASK_ID,
      async (_arg, signal) =>
        await new Promise<void>((resolve) => {
          signal.addEventListener("abort", () => resolve(), { once: true });
        }),
    );
    manager.start();

    const firstTaskRunId = scheduleCalendarSync(manager);
    await vi.advanceTimersByTimeAsync(100);
    const secondTaskRunId = scheduleCalendarSync(manager);

    expect(manager.getRunningTaskRunIds()).toContain(firstTaskRunId);
    expect(secondTaskRunId).toBe(firstTaskRunId);
  });

  test("can schedule a new calendar sync after a timeout", async () => {
    const manager = createTaskScheduler();
    managers.push(manager);
    manager.setTask(
      CALENDAR_SYNC_TASK_ID,
      async (_arg, signal) =>
        await new Promise<void>((resolve) => {
          signal.addEventListener("abort", () => resolve(), { once: true });
        }),
      { maxDuration: 50 },
    );
    manager.start();

    const timedOutTaskRunId = scheduleCalendarSync(manager);
    if (!timedOutTaskRunId) throw new Error("Failed to schedule calendar sync");
    await manager.untilTaskRunDone(timedOutTaskRunId);
    const nextTaskRunId = scheduleCalendarSync(manager);

    expect(manager.getTaskRunInfo(timedOutTaskRunId)).toBeUndefined();
    expect(nextTaskRunId).toBeDefined();
    expect(nextTaskRunId).not.toBe(timedOutTaskRunId);
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
    expect(writeQueueMocks.enqueueDatabaseWrite).toHaveBeenCalledWith(
      "calendar-sync",
      expect.any(Function),
    );
  });
});
