import { beforeEach, describe, expect, it, vi } from "vitest";

import { useTabs } from ".";
import { loadPinnedTabs, restorePinnedTabsToStore } from "./pinned-persistence";
import { resetTabsStore } from "./test-utils";

import { commands } from "~/types/tauri.gen";

describe("pinned tab persistence", () => {
  beforeEach(() => {
    resetTabsStore();
    vi.mocked(commands.getPinnedTabs).mockResolvedValue({
      status: "ok",
      data: null,
    });
  });

  it("drops legacy and classic-only pinned tabs during load", async () => {
    vi.mocked(commands.getPinnedTabs).mockResolvedValue({
      status: "ok",
      data: JSON.stringify([
        { type: "daily", pinned: true },
        { type: "empty", pinned: true },
        { type: "sessions", id: "session-1", pinned: true },
      ]),
    });

    const pinnedTabs = await loadPinnedTabs();

    expect(pinnedTabs).toMatchObject([{ type: "sessions", id: "session-1" }]);
  });

  it("restores supported tabs and ignores dropped daily and empty entries", async () => {
    vi.mocked(commands.getPinnedTabs).mockResolvedValue({
      status: "ok",
      data: JSON.stringify([
        { type: "daily", pinned: true },
        { type: "empty", pinned: true },
        { type: "sessions", id: "session-1", pinned: true },
      ]),
    });

    await restorePinnedTabsToStore(
      useTabs.getState().openNew,
      useTabs.getState().pin,
      () => useTabs.getState().tabs,
    );

    expect(useTabs.getState().tabs).toMatchObject([
      { type: "sessions", id: "session-1", pinned: true, active: true },
    ]);
  });
});
