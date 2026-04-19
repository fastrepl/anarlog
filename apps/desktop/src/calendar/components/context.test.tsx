import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const syncMocks = vi.hoisted(() => {
  let handler: null | ((event: { payload: Record<string, unknown> }) => void) =
    null;

  const attach = (nextHandler: typeof handler) => {
    handler = nextHandler;
    return Promise.resolve(() => {
      if (handler === nextHandler) {
        handler = null;
      }
    });
  };

  return {
    getCalendarSyncStatus: vi.fn(),
    requestCalendarSync: vi.fn(),
    attach,
    listen: vi.fn(attach),
    emit(payload: Record<string, unknown>) {
      handler?.({ payload });
    },
    reset() {
      handler = null;
    },
  };
});

vi.mock("@hypr/plugin-calendar", () => ({
  commands: {
    getCalendarSyncStatus: syncMocks.getCalendarSyncStatus,
    requestCalendarSync: syncMocks.requestCalendarSync,
  },
  events: {
    calendarSyncEvent: {
      listen: syncMocks.listen,
    },
  },
}));

import { SyncProvider, useSync } from "./context";

function StatusProbe() {
  const { status } = useSync();
  return <div data-testid="status">{status}</div>;
}

describe("SyncProvider", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    syncMocks.reset();
    syncMocks.getCalendarSyncStatus.mockReset();
    syncMocks.requestCalendarSync.mockReset();
    syncMocks.listen.mockReset();
    syncMocks.listen.mockImplementation(syncMocks.attach);
    syncMocks.requestCalendarSync.mockResolvedValue("idle");
    syncMocks.getCalendarSyncStatus.mockResolvedValue("idle");
  });

  test("waits for the sync listener before reading the initial status", async () => {
    let resolveListen: ((value: () => void) => void) | null = null;
    syncMocks.listen.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveListen = resolve;
        }),
    );

    render(
      <SyncProvider>
        <StatusProbe />
      </SyncProvider>,
    );

    expect(syncMocks.getCalendarSyncStatus).not.toHaveBeenCalled();

    act(() => {
      resolveListen?.(() => {});
    });

    await waitFor(() =>
      expect(syncMocks.getCalendarSyncStatus).toHaveBeenCalledTimes(1),
    );
  });

  test("ignores a stale initial status after live sync events arrive", async () => {
    let resolveStatus:
      | ((value: "idle" | "scheduled" | "running") => void)
      | null = null;
    syncMocks.getCalendarSyncStatus.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveStatus = resolve;
        }),
    );

    render(
      <SyncProvider>
        <StatusProbe />
      </SyncProvider>,
    );

    await waitFor(() => expect(syncMocks.listen).toHaveBeenCalledTimes(1));

    act(() => {
      syncMocks.emit({
        type: "syncStarted",
        reasons: ["manual"],
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("status").textContent).toBe("syncing");
    });

    act(() => {
      syncMocks.emit({
        type: "syncFinished",
        reasons: ["manual"],
        data_changed: false,
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId("status").textContent).toBe("idle");
    });

    act(() => {
      resolveStatus?.("running");
    });

    await waitFor(() => {
      expect(screen.getByTestId("status").textContent).toBe("idle");
    });
  });
});
