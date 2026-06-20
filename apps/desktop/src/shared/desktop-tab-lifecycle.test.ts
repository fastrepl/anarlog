import { describe, expect, it, vi } from "vitest";

import {
  createSessionTabCloseHandler,
  initializeDesktopTabs,
} from "./desktop-tab-lifecycle";

import {
  createContactsTab,
  createSessionTab,
} from "~/store/zustand/tabs/test-utils";

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
  });

  describe("createSessionTabCloseHandler", () => {
    it("cleans up empty sessions on close", () => {
      const invalidateSessionResource = vi.fn();
      const deleteSessionFn = vi.fn();
      const handler = createSessionTabCloseHandler({
        store: {} as Parameters<
          typeof createSessionTabCloseHandler
        >[0]["store"],
        indexes: {} as Parameters<
          typeof createSessionTabCloseHandler
        >[0]["indexes"],
        invalidateSessionResource,
        getSessionMode: vi.fn().mockReturnValue(null),
        isSessionEmptyFn: vi.fn().mockReturnValue(true),
        deleteSessionFn,
      });

      handler(createSessionTab({ id: "session-1" }));

      expect(invalidateSessionResource).toHaveBeenCalledWith("session-1");
      expect(deleteSessionFn).toHaveBeenCalledWith(
        expect.anything(),
        expect.anything(),
        "session-1",
        { deferFilesystemDelete: true },
      );
    });

    it("skips cleanup for non-inactive sessions and non-session tabs", () => {
      for (const sessionMode of ["active", "finalizing", "running_batch"]) {
        const invalidateSessionResource = vi.fn();
        const deleteSessionFn = vi.fn();
        const handler = createSessionTabCloseHandler({
          store: {} as Parameters<
            typeof createSessionTabCloseHandler
          >[0]["store"],
          indexes: {} as Parameters<
            typeof createSessionTabCloseHandler
          >[0]["indexes"],
          invalidateSessionResource,
          getSessionMode: vi.fn().mockReturnValue(sessionMode),
          isSessionEmptyFn: vi.fn().mockReturnValue(true),
          deleteSessionFn,
        });

        handler(createSessionTab({ id: `session-${sessionMode}` }));
        handler(createContactsTab());

        expect(invalidateSessionResource).not.toHaveBeenCalled();
        expect(deleteSessionFn).not.toHaveBeenCalled();
      }
    });
  });
});
