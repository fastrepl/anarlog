import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const saveMock = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
const relaunchMock = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
const flushDatabaseWritesWithinMock = vi
  .fn<() => Promise<void>>()
  .mockResolvedValue(undefined);
const getOnboardingNeededMock = vi
  .fn<() => Promise<{ status: "ok"; data: boolean }>>()
  .mockResolvedValue({ status: "ok", data: false });

vi.mock("@hypr/plugin-store2", () => ({
  commands: {
    save: saveMock,
  },
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: relaunchMock,
}));

vi.mock("~/db/write-queue", () => ({
  flushDatabaseWritesWithin: flushDatabaseWritesWithinMock,
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: {
    getOnboardingNeeded: getOnboardingNeededMock,
  },
}));

describe("automatic relaunch", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetModules();
    vi.clearAllMocks();
    flushDatabaseWritesWithinMock.mockResolvedValue(undefined);
    saveMock.mockResolvedValue(undefined);
    relaunchMock.mockResolvedValue(undefined);
    getOnboardingNeededMock.mockResolvedValue({ status: "ok", data: false });
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  test("schedules an immediate relaunch when onboarding is already done", async () => {
    const { scheduleAutomaticRelaunch } = await import("./relaunch");

    await expect(scheduleAutomaticRelaunch(2000)).resolves.toBe("scheduled");

    expect(saveMock).not.toHaveBeenCalled();
    expect(relaunchMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(2000);

    expect(saveMock).toHaveBeenCalledTimes(1);
    expect(flushDatabaseWritesWithinMock).toHaveBeenCalledWith(5000);
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });

  test("defers relaunch while onboarding is still required", async () => {
    getOnboardingNeededMock.mockResolvedValue({ status: "ok", data: true });
    const { scheduleAutomaticRelaunch } = await import("./relaunch");

    await expect(scheduleAutomaticRelaunch()).resolves.toBe("deferred");

    expect(saveMock).not.toHaveBeenCalled();
    expect(relaunchMock).not.toHaveBeenCalled();
  });

  test("still relaunches when flushing application state fails", async () => {
    const error = new Error("flush timed out");
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    flushDatabaseWritesWithinMock.mockRejectedValue(error);
    const { scheduleAutomaticRelaunch } = await import("./relaunch");

    await scheduleAutomaticRelaunch();
    await vi.runAllTimersAsync();

    expect(relaunchMock).toHaveBeenCalledOnce();
    expect(consoleError).toHaveBeenCalledWith(
      "Failed to flush application data before relaunch",
      error,
    );
    consoleError.mockRestore();
  });

  test("waits for settings to save when the database flush fails", async () => {
    let resolveSave!: () => void;
    flushDatabaseWritesWithinMock.mockRejectedValue(new Error("timed out"));
    saveMock.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveSave = resolve;
      }),
    );
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { scheduleAutomaticRelaunch } = await import("./relaunch");

    await scheduleAutomaticRelaunch();
    vi.advanceTimersByTime(0);
    await vi.waitFor(() => expect(saveMock).toHaveBeenCalledOnce());

    expect(relaunchMock).not.toHaveBeenCalled();
    resolveSave();
    await vi.runAllTimersAsync();
    expect(relaunchMock).toHaveBeenCalledOnce();
  });

  test("flushes a deferred relaunch after onboarding completes", async () => {
    getOnboardingNeededMock.mockResolvedValueOnce({
      status: "ok",
      data: true,
    });
    const { flushAutomaticRelaunch, scheduleAutomaticRelaunch } =
      await import("./relaunch");

    await scheduleAutomaticRelaunch();

    getOnboardingNeededMock.mockResolvedValue({ status: "ok", data: false });

    await expect(flushAutomaticRelaunch()).resolves.toBe(true);

    expect(saveMock).toHaveBeenCalledTimes(1);
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });
});
