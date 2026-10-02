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

vi.mock("~/auth/billing-context", () => ({
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

import { useClassicMainShortcuts } from "~/main/useShortcuts";

describe("useClassicMainShortcuts", () => {
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

    renderHook(() => useClassicMainShortcuts());

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

    const { result } = renderHook(() => useClassicMainShortcuts());

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

    const { result } = renderHook(() => useClassicMainShortcuts());

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

  it.each([
    [
      "a contentEditable editor",
      () => {
        const editor = document.createElement("div");
        editor.contentEditable = "true";
        editor.addEventListener("keydown", (event) => event.stopPropagation());
        document.body.append(editor);
        return { target: editor, cleanup: () => editor.remove() };
      },
    ],
    [
      "a ProseMirror text node",
      () => {
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
        return { target: text, cleanup: () => editor.remove() };
      },
    ],
    [
      "a session title input",
      () => {
        const input = document.createElement("input");
        input.dataset.sessionTitleInput = "true";
        input.addEventListener("keydown", (event) => event.preventDefault());
        document.body.append(input);
        return { target: input, cleanup: () => input.remove() };
      },
    ],
    [
      "a session surface child",
      () => {
        const surface = document.createElement("div");
        surface.dataset.sessionSurface = "true";
        const target = document.createElement("div");
        target.addEventListener("keydown", (event) => event.preventDefault());
        surface.append(target);
        document.body.append(surface);
        return { target, cleanup: () => surface.remove() };
      },
    ],
    [
      "a window listener",
      () => {
        const preventEscape = (event: KeyboardEvent) => {
          if (event.key === "Escape") {
            event.preventDefault();
          }
        };
        window.addEventListener("keydown", preventEscape);
        return {
          target: window,
          cleanup: () => window.removeEventListener("keydown", preventEscape),
        };
      },
    ],
  ] satisfies [string, () => { target: EventTarget; cleanup: () => void }][])(
    "opens home from a note when %s handles escape",
    (_label, setup) => {
      hoisted.currentTab = {
        active: true,
        slotId: "slot-session",
        type: "sessions",
      };

      renderHook(() => useClassicMainShortcuts());
      const { target, cleanup } = setup();
      dispatchEscape(target);
      vi.runOnlyPendingTimers();
      cleanup();

      expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
      expect(hoisted.select).not.toHaveBeenCalled();
    },
  );

  it("does not duplicate home navigation when a focused target handles escape directly", () => {
    hoisted.currentTab = {
      active: true,
      id: "session-1",
      slotId: "slot-session",
      type: "sessions",
    };
    const { result } = renderHook(() => useClassicMainShortcuts());
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

  it("does not duplicate chat close when a focused target handles escape directly", () => {
    hoisted.chatMode = "FloatingOpen";
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };
    const { result } = renderHook(() => useClassicMainShortcuts());
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

  it.each([
    [
      "an editor escape consumer",
      () => {
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
        return {
          target: editor,
          cleanup: () => {
            editor.remove();
            menu.remove();
          },
        };
      },
    ],
    [
      "an escape consumer unmounting during keydown",
      () => {
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
        return { target: editor, cleanup: () => editor.remove() };
      },
    ],
    [
      "a transcript editor removing itself",
      () => {
        const surface = document.createElement("div");
        surface.dataset.sessionSurface = "true";
        const editor = document.createElement("div");
        editor.dataset.transcriptEditor = "true";
        editor.contentEditable = "true";
        editor.addEventListener("keydown", (event) => {
          event.preventDefault();
          editor.remove();
        });
        surface.append(editor);
        document.body.append(surface);
        return { target: editor, cleanup: () => surface.remove() };
      },
    ],
    [
      "a focused input",
      () => {
        const input = document.createElement("input");
        input.addEventListener("keydown", (event) => event.preventDefault());
        document.body.append(input);
        input.focus();
        return { target: input, cleanup: () => input.remove() };
      },
    ],
  ] satisfies [string, () => { target: EventTarget; cleanup: () => void }][])(
    "lets %s consume escape",
    (_label, setup) => {
      hoisted.currentTab = {
        active: true,
        slotId: "slot-session",
        type: "sessions",
      };

      renderHook(() => useClassicMainShortcuts());
      const { target, cleanup } = setup();
      dispatchEscape(target);
      vi.runOnlyPendingTimers();
      cleanup();

      expect(hoisted.openCurrent).not.toHaveBeenCalled();
      expect(hoisted.select).not.toHaveBeenCalled();
    },
  );

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

    renderHook(() => useClassicMainShortcuts());

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

    renderHook(() => useClassicMainShortcuts());

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

    renderHook(() => useClassicMainShortcuts());

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

    renderHook(() => useClassicMainShortcuts());

    dispatchEscape();
    vi.runOnlyPendingTimers();

    expect(hoisted.openCurrent).toHaveBeenCalledWith({ type: "empty" });
    expect(hoisted.select).not.toHaveBeenCalled();
    expect(hoisted.goBack).not.toHaveBeenCalled();
  });

  it.each(["FloatingOpen", "RightPanelOpen"] as const)(
    "closes the %s chat before going home on escape",
    (chatMode) => {
      hoisted.chatMode = chatMode;
      hoisted.currentTab = {
        active: true,
        slotId: "slot-session",
        type: "sessions",
      };

      renderHook(() => useClassicMainShortcuts());

      dispatchEscape();
      vi.runOnlyPendingTimers();

      expect(hoisted.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
      expect(hoisted.openCurrent).not.toHaveBeenCalled();
    },
  );

  it("does not go home when chat closes before deferred escape handling runs", () => {
    hoisted.chatMode = "FloatingOpen";
    hoisted.currentTab = {
      active: true,
      slotId: "slot-session",
      type: "sessions",
    };

    const { rerender } = renderHook(() => useClassicMainShortcuts());

    dispatchEscape();
    hoisted.chatMode = "FloatingClosed";
    rerender();
    vi.runOnlyPendingTimers();

    expect(hoisted.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
    expect(hoisted.openCurrent).not.toHaveBeenCalled();
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
