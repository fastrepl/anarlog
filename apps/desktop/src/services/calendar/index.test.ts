import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const ctxMocks = vi.hoisted(() => ({
  createCtx: vi.fn(),
  getProviderConnections: vi.fn(),
  syncCalendars: vi.fn(),
}));

const fetchMocks = vi.hoisted(() => ({
  fetchIncomingEvents: vi.fn(),
}));

const storageMocks = vi.hoisted(() => ({
  syncConnectionEvents: vi.fn(),
  tombstoneCalendarConnection: vi.fn(),
}));

const writeQueueMocks = vi.hoisted(() => ({
  enqueueDatabaseWrite: vi.fn(),
}));

vi.mock("./ctx", () => ctxMocks);

vi.mock("./fetch", () => ({
  CalendarFetchError: class CalendarFetchError extends Error {},
  fetchIncomingEvents: fetchMocks.fetchIncomingEvents,
}));

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
    fetchMocks.fetchIncomingEvents.mockResolvedValue({
      events: [],
      participants: new Map(),
    });
    storageMocks.syncConnectionEvents.mockResolvedValue(undefined);
    storageMocks.tombstoneCalendarConnection.mockResolvedValue(undefined);
  });

  afterEach(() => {
    for (const manager of managers) {
      manager.stop();
    }
    managers.length = 0;
    vi.useRealTimers();
  });

  test.each([
    ["google-calendar", "conn-personal", "google"],
    ["apple", "apple", "apple"],
  ] as const)(
    "removes the exact disconnected %s connection",
    async (integration, connectionId, provider) => {
      await removeDisconnectedCalendarConnection(integration, connectionId);

      expect(storageMocks.tombstoneCalendarConnection).toHaveBeenCalledWith(
        provider,
        connectionId,
      );
      expect(ctxMocks.getProviderConnections).not.toHaveBeenCalled();
    },
  );

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

  test.each([
    [
      "range",
      (signal: AbortSignal) =>
        syncCalendarEventsForRange({ from: ctx.from, to: ctx.to }, { signal }),
    ],
    ["scheduled", (signal: AbortSignal) => syncCalendarEvents({ signal })],
  ])("does not start a %s sync when already aborted", async (_name, run) => {
    const abortController = new AbortController();
    abortController.abort();

    await run(abortController.signal);

    expect(ctxMocks.getProviderConnections).not.toHaveBeenCalled();
    expect(fetchMocks.fetchIncomingEvents).not.toHaveBeenCalled();
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

  test("reuses a scheduled or running calendar sync", async () => {
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

    const firstTaskRunId = scheduleCalendarSync(manager);
    expect(firstTaskRunId).toBeDefined();
    expect(scheduleCalendarSync(manager)).toBe(firstTaskRunId);

    manager.start();
    await vi.advanceTimersByTimeAsync(100);

    expect(manager.getRunningTaskRunIds()).toContain(firstTaskRunId);
    expect(scheduleCalendarSync(manager)).toBe(firstTaskRunId);
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
    expect(storageMocks.syncConnectionEvents).not.toHaveBeenCalled();
  });
});
