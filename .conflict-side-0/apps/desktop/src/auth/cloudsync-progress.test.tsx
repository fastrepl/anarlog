import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCloudsyncStatus: vi.fn(),
  showNotification: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  getCloudsyncStatus: mocks.getCloudsyncStatus,
}));

vi.mock("@hypr/plugin-notification", () => ({
  commands: {
    showNotification: mocks.showNotification,
  },
}));

import {
  startCloudsyncInitialSyncProgress,
  stopCloudsyncInitialSyncProgress,
  useCloudsyncInitialSyncProgress,
} from "./cloudsync-progress";

function cloudsyncStatus(lastSyncAtMs: number | null) {
  return {
    cloudsync_enabled: true,
    extension_loaded: true,
    configured: true,
    running: true,
    network_initialized: true,
    last_sync: lastSyncAtMs === null ? null : {},
    last_sync_at_ms: lastSyncAtMs,
    has_unsent_changes: false,
    last_error: null,
    last_error_kind: null,
    consecutive_failures: 0,
  };
}

describe("CloudSync initial sync progress", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    const storage = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      clear: () => storage.clear(),
      getItem: (key: string) => storage.get(key) ?? null,
      removeItem: (key: string) => storage.delete(key),
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    localStorage.clear();
    stopCloudsyncInitialSyncProgress();
    mocks.getCloudsyncStatus.mockReset();
    mocks.showNotification.mockReset();
    mocks.showNotification.mockResolvedValue({ status: "ok", data: null });
  });

  afterEach(() => {
    stopCloudsyncInitialSyncProgress();
    cleanup();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("shows progress and sends a notification when initial sync completes", async () => {
    mocks.getCloudsyncStatus
      .mockResolvedValueOnce(cloudsyncStatus(null))
      .mockResolvedValueOnce(cloudsyncStatus(123));
    const progress = renderHook(() => useCloudsyncInitialSyncProgress());

    act(() => startCloudsyncInitialSyncProgress("user-1"));

    expect(progress.result.current).toEqual({
      state: "syncing",
      toastId: "cloudsync-initial-sync-user-1",
      userId: "user-1",
    });

    await act(() => vi.advanceTimersByTimeAsync(2_000));

    expect(progress.result.current).toEqual({ state: "idle" });
    expect(
      localStorage.getItem("anarlog:cloudsync_initial_sync_completed:user-1"),
    ).toBe("1");
    expect(mocks.showNotification).toHaveBeenCalledWith(
      expect.objectContaining({
        key: "cloudsync-initial-sync-complete-user-1",
        title: "Cloud sync complete",
        message: "Your Anarlog data is ready on this device.",
      }),
    );
  });

  it("does not restart progress after completion was persisted", () => {
    localStorage.setItem(
      "anarlog:cloudsync_initial_sync_completed:user-1",
      "1",
    );
    const progress = renderHook(() => useCloudsyncInitialSyncProgress());

    act(() => startCloudsyncInitialSyncProgress("user-1"));

    expect(progress.result.current).toEqual({ state: "idle" });
    expect(mocks.getCloudsyncStatus).not.toHaveBeenCalled();
  });
});
