import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCloudsyncStatus: vi.fn(),
  playCompletionSound: vi.fn(),
  shouldShowNotification: vi.fn(),
  showNotification: vi.fn(),
}));

vi.mock("@anlg/plugin-db", () => ({
  getCloudsyncStatus: mocks.getCloudsyncStatus,
}));

vi.mock("@anlg/plugin-notification", () => ({
  commands: {
    showNotification: mocks.showNotification,
  },
}));

vi.mock("~/shared/completion-sound", () => ({
  playCompletionSound: mocks.playCompletionSound,
}));

vi.mock("~/shared/notification-policy", () => ({
  shouldShowNotification: mocks.shouldShowNotification,
}));

import {
  startCloudsyncInitialSyncProgress,
  stopCloudsyncInitialSyncProgress,
} from "./cloudsync-progress";

function cloudsyncStatus(lastSyncAtMs: number | null) {
  return {
    cloudsync_enabled: true,
    extension_loaded: true,
    configured: true,
    running: true,
    network_initialized: true,
    activity_paused: false,
    last_sync: lastSyncAtMs === null ? null : {},
    last_sync_at_ms: lastSyncAtMs,
    has_unsent_changes: false,
    last_error: null,
    last_error_kind: null,
    consecutive_failures: 0,
    deferred_for_capture: false,
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
    mocks.playCompletionSound.mockResolvedValue(undefined);
    mocks.shouldShowNotification.mockResolvedValue(true);
  });

  afterEach(() => {
    stopCloudsyncInitialSyncProgress();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("sends a notification when initial sync completes", async () => {
    mocks.getCloudsyncStatus
      .mockResolvedValueOnce(cloudsyncStatus(null))
      .mockResolvedValueOnce(cloudsyncStatus(123));

    startCloudsyncInitialSyncProgress("user-1");

    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.showNotification).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(2_000);

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
    expect(mocks.playCompletionSound).toHaveBeenCalledOnce();
  });

  it("does not restart monitoring after completion was persisted", () => {
    localStorage.setItem(
      "anarlog:cloudsync_initial_sync_completed:user-1",
      "1",
    );

    startCloudsyncInitialSyncProgress("user-1");

    expect(mocks.getCloudsyncStatus).not.toHaveBeenCalled();
    expect(mocks.showNotification).not.toHaveBeenCalled();
  });
});
