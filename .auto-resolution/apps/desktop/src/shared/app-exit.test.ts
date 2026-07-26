import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  completeAppExit: vi.fn().mockResolvedValue(undefined),
  flushDatabaseWritesWithin: vi.fn().mockResolvedValue(undefined),
  listener: null as (() => void) | null,
  save: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_event: string, listener: () => void) => {
    mocks.listener = listener;
    return vi.fn();
  }),
}));

vi.mock("@hypr/plugin-store2", () => ({
  commands: { save: mocks.save },
}));

vi.mock("~/db/write-queue", () => ({
  flushDatabaseWritesWithin: mocks.flushDatabaseWritesWithin,
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: { completeAppExit: mocks.completeAppExit },
}));

describe("initializeAppExitFlush", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    mocks.listener = null;
    mocks.completeAppExit.mockResolvedValue(undefined);
    mocks.flushDatabaseWritesWithin.mockResolvedValue(undefined);
    mocks.save.mockResolvedValue(undefined);
  });

  it("flushes queued writes and settings before completing exit", async () => {
    const { initializeAppExitFlush } = await import("./app-exit");
    await initializeAppExitFlush();

    mocks.listener?.();

    await vi.waitFor(() =>
      expect(mocks.completeAppExit).toHaveBeenCalledOnce(),
    );
    expect(mocks.flushDatabaseWritesWithin).toHaveBeenCalledWith(5000);
    expect(mocks.save).toHaveBeenCalledOnce();
    expect(
      mocks.flushDatabaseWritesWithin.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.completeAppExit.mock.invocationCallOrder[0]);
  });

  it("still exits when flushing fails", async () => {
    const error = new Error("write failed");
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.flushDatabaseWritesWithin.mockRejectedValue(error);
    const { initializeAppExitFlush } = await import("./app-exit");
    await initializeAppExitFlush();

    mocks.listener?.();

    await vi.waitFor(() =>
      expect(mocks.completeAppExit).toHaveBeenCalledOnce(),
    );
    expect(consoleError).toHaveBeenCalledWith(
      "Failed to flush application data before exit",
      error,
    );
    consoleError.mockRestore();
  });

  it("still exits when settings never finish saving", async () => {
    vi.useFakeTimers();
    mocks.save.mockReturnValueOnce(new Promise<void>(() => {}));
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { initializeAppExitFlush } = await import("./app-exit");
    await initializeAppExitFlush();

    mocks.listener?.();
    await vi.advanceTimersByTimeAsync(4999);
    expect(mocks.completeAppExit).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(mocks.completeAppExit).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });
});
