import { describe, expect, it, vi } from "vitest";

import {
  createDesktopTabCloseHandler,
  createSessionTabCloseHandler,
  initializeDesktopTabs,
} from "./desktop-tab-lifecycle";

import {
  createContactsTab,
  createSessionTab,
} from "~/store/zustand/tabs/test-utils";

const flushAsyncCleanup = () =>
  new Promise((resolve) => {
    setTimeout(resolve, 0);
  });

describe("desktop tab lifecycle", () => {
  describe("initializeDesktopTabs", () => {
    it("restores pinned tabs and recent notes without opening a startup tab", async () => {
      const tabs = [createSessionTab({ id: "restored-session" })];
      const openNew = vi.fn();
      const setRecentlyOpenedSessionIds = vi.fn();
      const restorePinnedTabs = vi.fn().mockResolvedValue(undefined);
      const restoreRecentlyOpenedSessionIds = vi
        .fn()
        .mockImplementation(async (set: (ids: string[]) => void) => {
          set(["restored-session"]);
        });

      await initializeDesktopTabs({
        getTabs: () => tabs,
        setRecentlyOpenedSessionIds,
        restorePinnedTabs,
        restoreRecentlyOpenedSessionIds,
        onZeroTabs: null,
        isTauriEnv: true,
      });

      expect(restorePinnedTabs).toHaveBeenCalledTimes(1);
      expect(setRecentlyOpenedSessionIds).toHaveBeenCalledWith([
        "restored-session",
      ]);
      expect(openNew).not.toHaveBeenCalled();
    });

    it("stays on home when startup has no restored tabs", async () => {
      const openNew = vi.fn();

      await initializeDesktopTabs({
        getTabs: () => [],
        setRecentlyOpenedSessionIds: vi.fn(),
        restorePinnedTabs: vi.fn().mockResolvedValue(undefined),
        restoreRecentlyOpenedSessionIds: vi.fn().mockResolvedValue(undefined),
        onZeroTabs: null,
        isTauriEnv: true,
      });

      expect(openNew).not.toHaveBeenCalled();
    });

    it("calls onZeroTabs when startup has no restored tabs", async () => {
      const openNew = vi.fn();
      const onZeroTabs = vi.fn();

      await initializeDesktopTabs({
        getTabs: () => [],
        setRecentlyOpenedSessionIds: vi.fn(),
        restorePinnedTabs: vi.fn().mockResolvedValue(undefined),
        restoreRecentlyOpenedSessionIds: vi.fn().mockResolvedValue(undefined),
        onZeroTabs,
        isTauriEnv: true,
      });

      expect(openNew).not.toHaveBeenCalled();
      expect(onZeroTabs).toHaveBeenCalledTimes(1);
    });

    it("runs startup work even when pinned tabs were restored", async () => {
      const tabs = [createSessionTab({ id: "restored-session" })];
      const onInitialized = vi.fn();
      const onZeroTabs = vi.fn();

      await initializeDesktopTabs({
        getTabs: () => tabs,
        setRecentlyOpenedSessionIds: vi.fn(),
        restorePinnedTabs: vi.fn().mockResolvedValue(undefined),
        restoreRecentlyOpenedSessionIds: vi.fn().mockResolvedValue(undefined),
        onInitialized,
        onZeroTabs,
        isTauriEnv: true,
      });

      expect(onInitialized).toHaveBeenCalledTimes(1);
      expect(onZeroTabs).not.toHaveBeenCalled();
    });
  });

  describe("createSessionTabCloseHandler", () => {
    it("cleans up empty sessions on close", async () => {
      const invalidateSessionResource = vi.fn();
      const deleteSessionFn = vi.fn().mockResolvedValue({ session: {} });
      const handler = createSessionTabCloseHandler({
        invalidateSessionResource,
        getSessionMode: vi.fn().mockReturnValue(null),
        isSessionEmptyFn: vi.fn().mockResolvedValue(true),
        deleteSessionFn,
      });

      handler(createSessionTab({ id: "session-1" }));

      await flushAsyncCleanup();

      expect(invalidateSessionResource).toHaveBeenCalledWith("session-1");
      expect(deleteSessionFn).toHaveBeenCalledWith("session-1");
    });

    it("keeps sessions that contain SQLite data", async () => {
      const invalidateSessionResource = vi.fn();
      const deleteSessionFn = vi.fn();
      const isSessionEmptyFn = vi.fn().mockResolvedValue(false);
      const handler = createSessionTabCloseHandler({
        invalidateSessionResource,
        getSessionMode: vi.fn().mockReturnValue(null),
        isSessionEmptyFn,
        deleteSessionFn,
      });

      handler(createSessionTab({ id: "session-1" }));

      await flushAsyncCleanup();

      expect(deleteSessionFn).not.toHaveBeenCalled();
      expect(invalidateSessionResource).not.toHaveBeenCalled();
    });

    it("does not invalidate when the SQLite delete loses a race", async () => {
      const invalidateSessionResource = vi.fn();
      const deleteSessionFn = vi.fn().mockResolvedValue(null);
      const handler = createSessionTabCloseHandler({
        invalidateSessionResource,
        getSessionMode: vi.fn().mockReturnValue(null),
        isSessionEmptyFn: vi.fn().mockResolvedValue(true),
        deleteSessionFn,
      });

      handler(createSessionTab({ id: "session-1" }));

      await flushAsyncCleanup();

      expect(deleteSessionFn).toHaveBeenCalledWith("session-1");
      expect(invalidateSessionResource).not.toHaveBeenCalled();
    });

    it("skips cleanup for non-inactive sessions and non-session tabs", () => {
      for (const sessionMode of ["active", "finalizing", "running_batch"]) {
        const invalidateSessionResource = vi.fn();
        const deleteSessionFn = vi.fn();
        const handler = createSessionTabCloseHandler({
          invalidateSessionResource,
          getSessionMode: vi.fn().mockReturnValue(sessionMode),
          isSessionEmptyFn: vi.fn().mockResolvedValue(true),
          deleteSessionFn,
        });

        handler(createSessionTab({ id: `session-${sessionMode}` }));
        handler(createContactsTab());

        expect(invalidateSessionResource).not.toHaveBeenCalled();
        expect(deleteSessionFn).not.toHaveBeenCalled();
      }
    });
  });

  it("purges ephemeral previews through the central close handler", () => {
    const purgePreview = vi.fn();
    const handler = createDesktopTabCloseHandler({
      invalidateSessionResource: vi.fn(),
      purgePreview,
    });

    handler({
      type: "shared_note_preview",
      id: "13697a87-f69b-456d-8679-4202d4f5d498",
      active: true,
      pinned: false,
      slotId: "slot-1",
    });

    expect(purgePreview).toHaveBeenCalledWith(
      "13697a87-f69b-456d-8679-4202d4f5d498",
    );
  });
});
