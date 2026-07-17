import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  completeAppExit: vi.fn().mockResolvedValue(undefined),
  flushDatabaseWrites: vi.fn().mockResolvedValue(undefined),
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
  flushDatabaseWrites: mocks.flushDatabaseWrites,
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
    mocks.flushDatabaseWrites.mockResolvedValue(undefined);
    mocks.save.mockResolvedValue(undefined);
  });

  it("flushes queued writes and settings before completing exit", async () => {
    const { initializeAppExitFlush } = await import("./app-exit");
    await initializeAppExitFlush();

    mocks.listener?.();

    await vi.waitFor(() =>
      expect(mocks.completeAppExit).toHaveBeenCalledOnce(),
    );
    expect(mocks.flushDatabaseWrites).toHaveBeenCalledOnce();
    expect(mocks.save).toHaveBeenCalledOnce();
    expect(mocks.flushDatabaseWrites.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.completeAppExit.mock.invocationCallOrder[0],
    );
  });

  it("still exits when flushing fails", async () => {
    const error = new Error("write failed");
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.flushDatabaseWrites.mockRejectedValue(error);
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
});
