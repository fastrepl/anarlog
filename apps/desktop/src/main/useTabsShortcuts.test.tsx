import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  chatMode: "FloatingClosed" as
    | "FloatingClosed"
    | "FloatingOpen"
    | "RightPanelOpen",
  currentTab: null as null | {
    active: boolean;
    id?: string;
    pinned?: boolean;
    returnToSlotId?: string;
    returnToTabId?: string;
    slotId: string;
    type: string;
  },
  canGoBack: false,
  close: vi.fn(),
  goBack: vi.fn(),
  handlers: new Map<string, (event?: { defaultPrevented: boolean }) => void>(),
  openCurrent: vi.fn(),
  openNew: vi.fn(),
  select: vi.fn(),
  sendEvent: vi.fn(),
  tabs: [] as {
    active: boolean;
    id?: string;
    returnToSlotId?: string;
    returnToTabId?: string;
    slotId: string;
    type: string;
  }[],
  unpin: vi.fn(),
  setPendingCloseConfirmationTab: vi.fn(),
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

vi.mock("~/store/zustand/tabs", () => {
  const getTabsState = () => ({
    tabs: hoisted.tabs,
    currentTab: hoisted.currentTab,
    canGoBack: hoisted.canGoBack,
    clearSelection: vi.fn(),
    close: hoisted.close,
    goBack: hoisted.goBack,
    select: hoisted.select,
    selectNext: vi.fn(),
    selectPrev: vi.fn(),
    restoreLastClosedTab: vi.fn(),
    openNew: hoisted.openNew,
    openCurrent: hoisted.openCurrent,
    unpin: hoisted.unpin,
    setPendingCloseConfirmationTab: hoisted.setPendingCloseConfirmationTab,
  });

  const useTabs = ((
    selector: (state: ReturnType<typeof getTabsState>) => unknown,
  ) => selector(getTabsState())) as ((
    selector: (state: ReturnType<typeof getTabsState>) => unknown,
  ) => unknown) & { getState: typeof getTabsState };

  useTabs.getState = getTabsState;

  return {
    uniqueIdfromTab: (tab: {
      id?: string;
      requestId?: string;
      slotId: string;
      type: string;
    }) => {
      switch (tab.type) {
        case "sessions":
        case "humans":
        case "organizations":
        case "task":
        case "daily_summary":
          return `${tab.type}-${tab.id}`;
        case "edit":
          return `edit-${tab.requestId}`;
        case "empty":
          return `empty-${tab.slotId}`;
        default:
          return tab.type;
      }
    },
    useTabs,
  };
});

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
    hoisted.close.mockClear();
    hoisted.currentTab = null;
    hoisted.canGoBack = false;
    hoisted.goBack.mockClear();
    hoisted.handlers.clear();
    hoisted.openCurrent.mockClear();
    hoisted.openNew.mockClear();
    hoisted.select.mockClear();
    hoisted.sendEvent.mockClear();
    hoisted.unpin.mockClear();
    hoisted.setPendingCloseConfirmationTab.mockClear();
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

  it("binds mod+w to close the current note tab", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };

    renderHook(() => useClassicMainTabsShortcuts());

    const handler = hoisted.handlers.get("mod+w");
    expect(handler).toBeTruthy();

    handler?.();

    expect(hoisted.close).toHaveBeenCalledWith(hoisted.currentTab);
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
  });

  it("selects an existing home view from a note on escape", () => {
    const homeTab = {
      active: false,
      slotId: "slot-home",
      type: "empty",
    };
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };
    hoisted.tabs = [homeTab, hoisted.currentTab];

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.select).toHaveBeenCalledWith(homeTab);
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
  });

  it("returns an escape shortcut action that opens home from a note", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };

    const { result } = renderHook(() => useClassicMainTabsShortcuts());

    result.current.runEscapeShortcut();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("uses the latest tab state when the returned escape action runs", () => {
    hoisted.currentTab = {
      active: true,
      slotId: "slot-home",
      type: "empty",
    };

    const { result } = renderHook(() => useClassicMainTabsShortcuts());

    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };

    result.current.runEscapeShortcut();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("opens home from a note when the editor stops escape propagation", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
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
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("opens home from a note when the editor prevents default escape handling", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };
    const editor = document.createElement("div");
    editor.className = "ProseMirror";
    editor.contentEditable = "true";
    editor.addEventListener("keydown", (event) => {
      event.preventDefault();
      event.stopPropagation();
    });
    document.body.append(editor);

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape(editor);
    vi.runOnlyPendingTimers();
    editor.remove();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("opens home from a note when ProseMirror handles escape from a text node", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };
    const editor = document.createElement("div");
    editor.className = "ProseMirror";
    editor.contentEditable = "true";
    const text = document.createTextNode("note");
    editor.append(text);
    editor.addEventListener("keydown", (event) => {
      event.preventDefault();
      event.stopPropagation();
    });
    document.body.append(editor);

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape(text);
    vi.runOnlyPendingTimers();
    editor.remove();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("does not duplicate home navigation when a focused target handles escape directly", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };
    const { result } = renderHook(() => useClassicMainTabsShortcuts());
    const target = document.createElement("input");
    target.addEventListener("keydown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      result.current.runEscapeShortcut();
    });
    document.body.append(target);

    dispatchEscape(target);
    vi.runOnlyPendingTimers();
    target.remove();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.openCurrent).toHaveBeenCalledTimes(1);
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("opens home from a note when the title field handles escape", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };
    const target = document.createElement("input");
    target.dataset.sessionTitleInput = "true";
    target.addEventListener("keydown", (event) => {
      event.preventDefault();
    });
    document.body.append(target);

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape(target);
    vi.runOnlyPendingTimers();
    target.remove();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("opens home from a note when the session surface handles escape", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };
    const surface = document.createElement("div");
    surface.dataset.sessionSurface = "true";
    const target = document.createElement("div");
    target.addEventListener("keydown", (event) => {
      event.preventDefault();
    });
    surface.append(target);
    document.body.append(surface);

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape(target);
    vi.runOnlyPendingTimers();
    surface.remove();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("does not duplicate chat close when a focused target handles escape directly", () => {
    hoisted.chatMode = "FloatingOpen";
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };
    const { result } = renderHook(() => useClassicMainTabsShortcuts());
    const target = document.createElement("input");
    target.addEventListener("keydown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      result.current.runEscapeShortcut();
    });
    document.body.append(target);

    dispatchEscape(target);
    vi.runOnlyPendingTimers();
    target.remove();

    expect(hoisted.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
    expect(hoisted.sendEvent).toHaveBeenCalledTimes(1);
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
  });

  it("lets editor escape consumers handle the key before opening home", () => {
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };
    const editor = document.createElement("div");
    editor.className = "ProseMirror";
    editor.contentEditable = "true";
    editor.addEventListener("keydown", (event) => {
      event.preventDefault();
      event.stopPropagation();
    });
    const menu = document.createElement("div");
    menu.dataset.editorEscapeConsumer = "true";
    document.body.append(editor, menu);

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape(editor);
    vi.runOnlyPendingTimers();
    editor.remove();
    menu.remove();

    expect(hoisted.openCurrent).not.toHaveBeenCalled();
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("does not open home when an editor escape consumer unmounts before the shortcut runs", () => {
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };
    const editor = document.createElement("div");
    editor.className = "ProseMirror";
    editor.contentEditable = "true";
    const menu = document.createElement("div");
    menu.dataset.editorEscapeConsumer = "true";
    editor.addEventListener("keydown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      menu.remove();
    });
    document.body.append(editor, menu);

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape(editor);
    vi.runOnlyPendingTimers();
    editor.remove();

    expect(hoisted.openCurrent).not.toHaveBeenCalled();
    expect(hoisted.select).not.toHaveBeenCalled();
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

  it("returns settings to the tab it opened from on escape", () => {
    const sessionTab = {
      active: false,
      slotId: "slot-session",
      type: "sessions",
    };
    hoisted.currentTab = {
      active: true,
      returnToSlotId: "slot-session",
      slotId: "slot-settings",
      type: "settings",
    };
    hoisted.tabs = [
      sessionTab,
      {
        active: false,
        slotId: "slot-home",
        type: "empty",
      },
      hoisted.currentTab,
    ];

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.select).toHaveBeenCalledWith(sessionTab);
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
    expect(hoisted.goBack).not.toHaveBeenCalled();
  });

  it("uses history when an origin-tracked tab replaced the current slot", () => {
    hoisted.currentTab = {
      active: true,
      returnToSlotId: "slot-session",
      slotId: "slot-session",
      type: "settings",
    };
    hoisted.canGoBack = true;
    hoisted.tabs = [hoisted.currentTab];

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.goBack).toHaveBeenCalledTimes(1);
    expect(hoisted.select).not.toHaveBeenCalled();
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
  });

  it("opens home when the return origin is gone instead of using unrelated history", () => {
    hoisted.currentTab = {
      active: true,
      returnToSlotId: "slot-session",
      slotId: "slot-settings",
      type: "settings",
    };
    hoisted.canGoBack = true;
    hoisted.tabs = [hoisted.currentTab];

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.goBack).not.toHaveBeenCalled();
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("opens home when the origin slot now contains a different tab", () => {
    const reusedSlotTab = {
      active: false,
      id: "new-session",
      slotId: "slot-session",
      type: "sessions",
    };
    hoisted.currentTab = {
      active: true,
      returnToSlotId: "slot-session",
      returnToTabId: "sessions-old-session",
      slotId: "slot-settings",
      type: "settings",
    };
    hoisted.tabs = [reusedSlotTab, hoisted.currentTab];

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
    expect(hoisted.goBack).not.toHaveBeenCalled();
  });

  it("closes the floating chat before going home on escape", () => {
    hoisted.chatMode = "FloatingOpen";
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

  it("does not go home when chat closes before deferred escape handling runs", () => {
    hoisted.chatMode = "FloatingOpen";
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };

    const { rerender } = renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape();
    hoisted.chatMode = "FloatingClosed";
    rerender();
    vi.runOnlyPendingTimers();

    expect(hoisted.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
  });

  it("closes the right panel chat before going home on escape", () => {
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

  it("opens home from a note when escape is prevented without a focused target", () => {
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

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
  });

  it("lets focused targets consume escape", () => {
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };
    const input = document.createElement("input");
    input.addEventListener("keydown", (event) => {
      event.preventDefault();
    });
    document.body.append(input);
    input.focus();

    renderHook(() => useClassicMainTabsShortcuts());

    dispatchEscape(input);
    vi.runOnlyPendingTimers();
    input.remove();

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
