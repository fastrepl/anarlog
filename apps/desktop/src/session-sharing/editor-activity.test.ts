import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  currentTab: null as null | { type: string; id?: string },
  flushDatabaseWrites: vi.fn<() => Promise<void>>(),
  getAllWebviewWindows: vi.fn<() => Promise<Array<{ label: string }>>>(),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getAllWebviewWindows: mocks.getAllWebviewWindows,
}));

vi.mock("~/db/write-queue", () => ({
  flushDatabaseWrites: mocks.flushDatabaseWrites,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: {
    getState: () => ({ currentTab: mocks.currentTab }),
  },
}));

import {
  beginCanonicalSessionEditorActivation,
  flushCanonicalSessionEditorChanges,
  isCanonicalSessionEditorActive,
  isCanonicalSessionImportLocked,
  registerCanonicalSessionEditor,
  tryAcquireCanonicalSessionImportLock,
  unregisterCanonicalSessionEditor,
} from "./editor-activity";

describe("canonical session editor activity", () => {
  beforeEach(() => {
    mocks.currentTab = null;
    mocks.flushDatabaseWrites.mockReset();
    mocks.flushDatabaseWrites.mockResolvedValue();
    mocks.getAllWebviewWindows.mockReset();
    mocks.getAllWebviewWindows.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("treats a mounted focused editor as active", async () => {
    const view = { hasFocus: () => true } as never;
    registerCanonicalSessionEditor("session-1", view, vi.fn());

    await expect(isCanonicalSessionEditorActive("session-1")).resolves.toBe(
      true,
    );
    expect(mocks.getAllWebviewWindows).not.toHaveBeenCalled();

    unregisterCanonicalSessionEditor("session-1", view);
  });

  it("keeps an unfocused mounted editor active through its pending 500ms write", async () => {
    vi.useFakeTimers();
    const view = { hasFocus: () => false } as never;
    const persistPendingChange = vi.fn();
    registerCanonicalSessionEditor("session-1", view, vi.fn());
    setTimeout(persistPendingChange, 500);

    await vi.advanceTimersByTimeAsync(499);

    await expect(isCanonicalSessionEditorActive("session-1")).resolves.toBe(
      true,
    );
    expect(persistPendingChange).not.toHaveBeenCalled();

    unregisterCanonicalSessionEditor("session-1", view);
  });

  it("detects the current session tab before its editor view mounts", async () => {
    mocks.currentTab = { type: "sessions", id: "session-1" };

    await expect(isCanonicalSessionEditorActive("session-1")).resolves.toBe(
      true,
    );
    expect(mocks.getAllWebviewWindows).not.toHaveBeenCalled();
  });

  it("forces every mounted editor change before draining database writes", async () => {
    const firstView = { hasFocus: () => true } as never;
    const secondView = { hasFocus: () => false } as never;
    const firstFlush = vi.fn();
    const secondFlush = vi.fn();
    registerCanonicalSessionEditor("session-1", firstView, firstFlush);
    registerCanonicalSessionEditor("session-1", secondView, secondFlush);
    mocks.flushDatabaseWrites.mockImplementationOnce(async () => {
      expect(firstFlush).toHaveBeenCalledOnce();
      expect(secondFlush).toHaveBeenCalledOnce();
    });

    await flushCanonicalSessionEditorChanges("session-1");

    expect(mocks.flushDatabaseWrites).toHaveBeenCalledOnce();
    unregisterCanonicalSessionEditor("session-1", firstView);
    unregisterCanonicalSessionEditor("session-1", secondView);
  });

  it("excludes editor activation for the full import lease", () => {
    const releaseImport = tryAcquireCanonicalSessionImportLock("session-1");
    expect(releaseImport).not.toBeNull();
    expect(isCanonicalSessionImportLocked("session-1")).toBe(true);

    expect(beginCanonicalSessionEditorActivation("session-1")).toBeNull();

    releaseImport?.();
    expect(isCanonicalSessionImportLocked("session-1")).toBe(false);
    const finishActivation = beginCanonicalSessionEditorActivation("session-1");
    expect(finishActivation).not.toBeNull();
    finishActivation?.();
  });

  it("detects an editor in a standalone note window", async () => {
    mocks.getAllWebviewWindows.mockResolvedValue([
      { label: "main" },
      { label: "note-session-1" },
    ]);

    await expect(isCanonicalSessionEditorActive("session-1")).resolves.toBe(
      true,
    );
  });

  it("fails closed when standalone window activity cannot be read", async () => {
    mocks.getAllWebviewWindows.mockRejectedValue(new Error("unavailable"));

    await expect(isCanonicalSessionEditorActive("session-1")).resolves.toBe(
      true,
    );
  });
});
