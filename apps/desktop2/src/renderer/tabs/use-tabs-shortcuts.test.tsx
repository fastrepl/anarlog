import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  handlers: new Map<string, (event: { key: string }) => void>(),
}));

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: (keys: string, handler: (event: { key: string }) => void) => {
    hoisted.handlers.set(keys, handler);
  },
}));

import { selectCurrentTab, useTabsShortcuts, useTabsStore } from "~/tabs";

describe("useTabsShortcuts", () => {
  beforeEach(() => {
    hoisted.handlers.clear();
    useTabsStore.setState({
      tabs: [{ type: "sessions", id: "session-1" }, { type: "settings" }],
      currentId: "sessions-session-1",
      history: [null, "sessions-session-1"],
      historyIndex: 1,
    });
  });

  it("binds mod+w to close the selected tab", () => {
    renderHook(() => useTabsShortcuts());

    const handler = hoisted.handlers.get("mod+w");
    expect(handler).toBeTruthy();

    handler?.({ key: "w" });

    expect(useTabsStore.getState().tabs).toEqual([{ type: "settings" }]);
    expect(selectCurrentTab(useTabsStore.getState())).toEqual({
      type: "settings",
    });
  });

  it("binds mod+1..9 to select the indexed tab", () => {
    renderHook(() => useTabsShortcuts());

    const handler = hoisted.handlers.get(
      "mod+1, mod+2, mod+3, mod+4, mod+5, mod+6, mod+7, mod+8, mod+9",
    );
    expect(handler).toBeTruthy();

    handler?.({ key: "2" });

    expect(selectCurrentTab(useTabsStore.getState())).toEqual({
      type: "settings",
    });
  });

  it("binds mod+t to clear the current selection", () => {
    renderHook(() => useTabsShortcuts());

    const handler = hoisted.handlers.get("mod+t");
    expect(handler).toBeTruthy();

    handler?.({ key: "t" });

    expect(useTabsStore.getState().currentId).toBeNull();
  });

  it("binds mod+shift+t to restore the last closed tab", () => {
    renderHook(() => useTabsShortcuts());

    const closeHandler = hoisted.handlers.get("mod+w");
    closeHandler?.({ key: "w" });
    expect(useTabsStore.getState().tabs).toEqual([{ type: "settings" }]);

    const restoreHandler = hoisted.handlers.get("mod+shift+t");
    expect(restoreHandler).toBeTruthy();

    restoreHandler?.({ key: "t" });

    expect(useTabsStore.getState().tabs).toEqual([
      { type: "settings" },
      { type: "sessions", id: "session-1" },
    ]);
    expect(selectCurrentTab(useTabsStore.getState())).toEqual({
      type: "sessions",
      id: "session-1",
    });
  });

  it("binds mod+n to the onNewNote callback", () => {
    const onNewNote = vi.fn();
    renderHook(() => useTabsShortcuts({ onNewNote }));

    hoisted.handlers.get("mod+n")?.({ key: "n" });

    expect(onNewNote).toHaveBeenCalledTimes(1);
  });

  it("binds mod+shift+c to open a calendar tab", () => {
    renderHook(() => useTabsShortcuts());

    hoisted.handlers.get("mod+shift+c")?.({ key: "c" });

    expect(selectCurrentTab(useTabsStore.getState())).toEqual({
      type: "calendar",
    });
  });
});
