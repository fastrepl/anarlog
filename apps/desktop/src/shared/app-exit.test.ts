import type { EditorView } from "prosemirror-view";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  analyticsEvent: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  completeAppExit: vi.fn().mockResolvedValue(undefined),
  flushDatabaseWritesWithin: vi.fn<(timeoutMs: number) => Promise<void> | undefined>(),
  listener: null as
    | ((event: { payload: { quick?: boolean } | null }) => void)
    | null,
  save: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (_event: string, listener: NonNullable<typeof mocks.listener>) => {
      mocks.listener = listener;
      return vi.fn();
    },
  ),
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: { event: mocks.analyticsEvent },
}));

vi.mock("@anlg/plugin-store2", () => ({
  commands: { save: mocks.save },
}));

// The real queue, unless a test gives the mock its own result.
vi.mock("~/db/write-queue", async (importOriginal) => {
  const actual = await importOriginal<typeof import("~/db/write-queue")>();
  return {
    ...actual,
    flushDatabaseWritesWithin: (timeoutMs: number) =>
      mocks.flushDatabaseWritesWithin(timeoutMs) ??
      actual.flushDatabaseWritesWithin(timeoutMs),
  };
});

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getAllWebviewWindows: vi.fn().mockResolvedValue([]),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: { getState: () => ({ currentTab: null }) },
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: { completeAppExit: mocks.completeAppExit },
}));

describe("initializeAppExitFlush", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    mocks.flushDatabaseWritesWithin.mockReset();
    mocks.listener = null;
    mocks.analyticsEvent.mockResolvedValue({ status: "ok", data: null });
    mocks.completeAppExit.mockResolvedValue(undefined);
    mocks.save.mockResolvedValue(undefined);
  });

  it("flushes queued writes and settings before completing exit", async () => {
    const { initializeAppExitFlush } = await import("./app-exit");
    await initializeAppExitFlush();

    mocks.listener?.({ payload: null });

    await vi.waitFor(() =>
      expect(mocks.completeAppExit).toHaveBeenCalledOnce(),
    );
    expect(mocks.analyticsEvent).toHaveBeenCalledWith({
      event: "app_exit_requested",
    });
    expect(mocks.flushDatabaseWritesWithin).toHaveBeenCalledWith(5000);
    expect(mocks.save).toHaveBeenCalledOnce();
    expect(
      mocks.flushDatabaseWritesWithin.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.completeAppExit.mock.invocationCallOrder[0]);
  });

  it("saves an editor's unsaved edit before a confirmed quit exits", async () => {
    const { initializeAppExitFlush } = await import("./app-exit");
    const { enqueueDatabaseWrite } = await import("~/db/write-queue");
    const { registerCanonicalSessionEditor } =
      await import("~/session-sharing/editor-activity");
    const saved: string[] = [];
    // Like the note editor: its last keys wait in a 500 ms debounce until flushed.
    registerCanonicalSessionEditor("note", {} as EditorView, () => {
      void enqueueDatabaseWrite("session:note", async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
        saved.push("last keys");
      });
    });
    let savedAtExit: string[] = [];
    mocks.completeAppExit.mockImplementation(async () => {
      savedAtExit = [...saved];
    });
    await initializeAppExitFlush();

    mocks.listener?.({ payload: { quick: true } });

    await vi.waitFor(() =>
      expect(mocks.completeAppExit).toHaveBeenCalledOnce(),
    );
    expect(savedAtExit).toEqual(["last keys"]);
  });

  it("still exits when flushing fails", async () => {
    const error = new Error("write failed");
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.flushDatabaseWritesWithin.mockRejectedValue(error);
    const { initializeAppExitFlush } = await import("./app-exit");
    await initializeAppExitFlush();

    mocks.listener?.({ payload: null });

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

    mocks.listener?.({ payload: null });
    await vi.advanceTimersByTimeAsync(4999);
    expect(mocks.completeAppExit).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(mocks.completeAppExit).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });
});
