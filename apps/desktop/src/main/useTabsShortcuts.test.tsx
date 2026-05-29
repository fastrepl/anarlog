import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as "FloatingClosed" | "RightPanelOpen",
  currentTab: null as null | { active: boolean; slotId: string; type: string },
  handlers: new Map<string, (event?: { defaultPrevented: boolean }) => void>(),
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  select: vi.fn(),
  sendEvent: vi.fn(),
  tabs: [] as { active: boolean; slotId: string; type: string }[],
}));

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: (
    keys: string,
    handler: (event?: { defaultPrevented: boolean }) => void,
  ) => {
    hoisted.handlers.set(keys, handler);
  },
}));

vi.mock("~/auth/billing", () => ({
  useBillingAccess: () => ({ isPro: true }),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: hoisted.chatMode,
      sendEvent: hoisted.sendEvent,
      startNewChat: vi.fn(),
    },
  }),
}));

vi.mock("~/shared/useNewNote", () => ({
  useNewNote: () => vi.fn(),
  useNewNoteAndListen: () => vi.fn(),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      clearSelection: () => void;
      close: () => void;
      currentTab: typeof hoisted.currentTab;
      openCurrent: typeof hoisted.openCurrent;
      openNew: typeof hoisted.openNew;
      restoreLastClosedTab: () => void;
      select: typeof hoisted.select;
      selectNext: () => void;
      selectPrev: () => void;
      setPendingCloseConfirmationTab: () => void;
      tabs: typeof hoisted.tabs;
      unpin: () => void;
    }) => unknown,
  ) =>
    selector({
      tabs: hoisted.tabs,
      currentTab: hoisted.currentTab,
      clearSelection: vi.fn(),
      close: vi.fn(),
      select: hoisted.select,
      selectNext: vi.fn(),
      selectPrev: vi.fn(),
      restoreLastClosedTab: vi.fn(),
      openNew: hoisted.openNew,
      openCurrent: hoisted.openCurrent,
      unpin: vi.fn(),
      setPendingCloseConfirmationTab: vi.fn(),
    }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: { sessionId: string | null; status: string };
    }) => unknown,
  ) =>
    selector({
      live: { sessionId: null, status: "idle" },
    }),
}));

import { useClassicMainTabsShortcuts } from "~/main/useTabsShortcuts";

describe("useClassicMainTabsShortcuts", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    hoisted.chatMode = "FloatingClosed";
    hoisted.currentTab = null;
    hoisted.handlers.clear();
    hoisted.openCurrent.mockClear();
    hoisted.openNew.mockClear();
    hoisted.select.mockClear();
    hoisted.sendEvent.mockClear();
    hoisted.tabs = [];
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("binds mod+t to open a classic empty tab", () => {
    renderHook(() => useClassicMainTabsShortcuts());

    const handler = hoisted.handlers.get("mod+t");
    expect(handler).toBeTruthy();

    handler?.();

    expect(hoisted.openNew).toHaveBeenCalledWith({ type: "empty" });
  });

  it("binds escape to open the home view", () => {
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
  });

  it("opens the home view even when the editor stops escape propagation", () => {
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };
    const editor = document.createElement("div");
    editor.contentEditable = "true";
    editor.addEventListener("keydown", (event) => event.stopPropagation());
    document.body.append(editor);

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape(editor);
    vi.runOnlyPendingTimers();
    editor.remove();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
  });

  it("selects an existing home tab on escape", () => {
    const homeTab = {
      active: false,
      slotId: "slot-home",
      type: "empty",
    };
    hoisted.currentTab = {
      active: true,
      slotId: "slot-settings",
      type: "settings",
    };
    hoisted.tabs = [homeTab, hoisted.currentTab];

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.select).toHaveBeenCalledWith(homeTab);
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
  });

  it("closes the chat panel before going home on escape", () => {
    hoisted.chatMode = "RightPanelOpen";
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
  });

  it("lets earlier escape handlers consume the key", () => {
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };

    renderHook(() => useClassicMainTabsShortcuts());

    const preventEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
      }
    };
    window.addEventListener("keydown", preventEscape);

    dispatchEscape();
    vi.runOnlyPendingTimers();
    window.removeEventListener("keydown", preventEscape);

    expect(hoisted.openCurrent).not.toHaveBeenCalled();
    expect(hoisted.select).not.toHaveBeenCalled();
  });
});

function dispatchEscape(target: EventTarget = window) {
  target.dispatchEvent(
    new KeyboardEvent("keydown", {
      bubbles: true,
      cancelable: true,
      key: "Escape",
    }),
  );
}
