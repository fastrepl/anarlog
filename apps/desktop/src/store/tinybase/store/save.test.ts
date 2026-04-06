import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const saveMock = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
const relaunchMock = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
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
    getOnboardingNeededMock.mockResolvedValue({ status: "ok", data: false });
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  test("schedules an immediate relaunch when onboarding is already done", async () => {
    const { scheduleAutomaticRelaunch } = await import("./save");

    await expect(scheduleAutomaticRelaunch(2000)).resolves.toBe("scheduled");

    expect(saveMock).not.toHaveBeenCalled();
    expect(relaunchMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(2000);

    expect(saveMock).toHaveBeenCalledTimes(1);
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });

  test("defers relaunch while onboarding is still required", async () => {
    getOnboardingNeededMock.mockResolvedValue({ status: "ok", data: true });
    const { scheduleAutomaticRelaunch } = await import("./save");

    await expect(scheduleAutomaticRelaunch()).resolves.toBe("deferred");

    expect(saveMock).not.toHaveBeenCalled();
    expect(relaunchMock).not.toHaveBeenCalled();
  });

  test("flushes a deferred relaunch after onboarding completes", async () => {
    getOnboardingNeededMock.mockResolvedValueOnce({
      status: "ok",
      data: true,
    });
    const { flushAutomaticRelaunch, scheduleAutomaticRelaunch } =
      await import("./save");

    await scheduleAutomaticRelaunch();

    getOnboardingNeededMock.mockResolvedValue({ status: "ok", data: false });

    await expect(flushAutomaticRelaunch()).resolves.toBe(true);

    expect(saveMock).toHaveBeenCalledTimes(1);
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });
});
