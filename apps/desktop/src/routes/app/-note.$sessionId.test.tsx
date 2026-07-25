import {
  act,
  cleanup,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { resetTabsStore } from "~/store/zustand/tabs/test-utils";

const mocks = vi.hoisted(() => ({
  attachLiveSession: vi.fn(),
  chatMode: "FloatingClosed" as
    | "FloatingClosed"
    | "FloatingOpen"
    | "RightPanelOpen",
  close: vi.fn(),
  listenerState: {
    attachLiveSession: vi.fn(),
    live: {
      eventUnlistenersBySession: {} as Record<string, (() => void)[]>,
    },
  },
  sendEvent: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: mocks.close,
  }),
}));

vi.mock("~/main/layout", () => ({
  ClassicMainLayout: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: mocks.chatMode,
      sendEvent: mocks.sendEvent,
    },
  }),
}));

vi.mock("~/session", () => ({
  TabContentNote: ({ standaloneWindow }: { standaloneWindow?: boolean }) => (
    <div
      data-testid="standalone-note"
      data-standalone-window={standaloneWindow}
    />
  ),
}));

vi.mock("~/shared/main", () => ({
  MainChatPanels: ({
    autoSaveId,
    children,
    leftSidebarAvailable,
    noteSurfaceMinWidth,
  }: {
    autoSaveId?: string;
    children: React.ReactNode;
    leftSidebarAvailable?: boolean;
    noteSurfaceMinWidth?: number;
  }) => (
    <div
      data-auto-save-id={autoSaveId}
      data-left-sidebar-available={leftSidebarAvailable}
      data-note-surface-min-width={noteSurfaceMinWidth}
      data-testid="standalone-chat-panels"
    >
      {children}
    </div>
  ),
}));

vi.mock("~/shared/window-shell", () => ({
  StandaloneWindowShell: ({ children }: { children: React.ReactNode }) =>
    children,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: typeof mocks.listenerState) => unknown) =>
    selector(mocks.listenerState),
}));

import {
  Route,
  StandaloneNoteWindow,
  useAttachStandaloneNoteToLiveSession,
  useCloseStandaloneNoteWindowOnEscape,
  useStandaloneNoteTab,
} from "./note.$sessionId";

import { useTabs } from "~/store/zustand/tabs";

describe("standalone note window route", () => {
  beforeEach(() => {
    mocks.listenerState.attachLiveSession = mocks.attachLiveSession;
    mocks.listenerState.live.eventUnlistenersBySession = {};
    mocks.chatMode = "FloatingClosed";
    mocks.attachLiveSession.mockClear();
    mocks.close.mockClear();
    mocks.sendEvent.mockClear();
    resetTabsStore();
  });

  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  it("closes the standalone note window on escape", () => {
    vi.useFakeTimers();
    renderHook(() => useCloseStandaloneNoteWindowOnEscape());

    const event = dispatchKeyDown("Escape");
    act(() => vi.runOnlyPendingTimers());

    expect(event.defaultPrevented).toBe(true);
    expect(mocks.close).toHaveBeenCalledTimes(1);
  });

  it("ignores other keys", () => {
    renderHook(() => useCloseStandaloneNoteWindowOnEscape());

    const event = dispatchKeyDown("Enter");

    expect(event.defaultPrevented).toBe(false);
    expect(mocks.close).not.toHaveBeenCalled();
  });

  it("closes chat before closing the standalone note window", () => {
    vi.useFakeTimers();
    mocks.chatMode = "FloatingOpen";
    renderHook(() => useCloseStandaloneNoteWindowOnEscape());

    const event = dispatchKeyDown("Escape");
    act(() => vi.runOnlyPendingTimers());

    expect(event.defaultPrevented).toBe(true);
    expect(mocks.sendEvent).toHaveBeenCalledWith({ type: "CLOSE" });
    expect(mocks.close).not.toHaveBeenCalled();
  });

  it("lets an overlay consume escape before closing chat", () => {
    vi.useFakeTimers();
    mocks.chatMode = "FloatingOpen";
    const overlay = document.createElement("button");
    overlay.addEventListener("keydown", (event) => event.preventDefault());
    document.body.append(overlay);
    renderHook(() => useCloseStandaloneNoteWindowOnEscape());

    const event = dispatchKeyDown("Escape", overlay);
    act(() => vi.runOnlyPendingTimers());
    overlay.remove();

    expect(event.defaultPrevented).toBe(true);
    expect(mocks.sendEvent).not.toHaveBeenCalled();
    expect(mocks.close).not.toHaveBeenCalled();
  });

  it("mounts chat panels around the standalone note", () => {
    vi.spyOn(Route, "useParams").mockReturnValue({
      sessionId: "session-1",
    });

    render(<StandaloneNoteWindow />);

    expect(
      screen
        .getByTestId("standalone-chat-panels")
        .contains(screen.getByTestId("standalone-note")),
    ).toBe(true);
    expect(
      screen
        .getByTestId("standalone-note")
        .getAttribute("data-standalone-window"),
    ).toBe("true");
    expect(
      screen
        .getByTestId("standalone-chat-panels")
        .getAttribute("data-left-sidebar-available"),
    ).toBe("false");
    expect(
      screen
        .getByTestId("standalone-chat-panels")
        .getAttribute("data-note-surface-min-width"),
    ).toBe("420");
    expect(
      screen
        .getByTestId("standalone-chat-panels")
        .getAttribute("data-auto-save-id"),
    ).toBe("standalone-note-chat");
  });

  it("returns the subscribed standalone note tab after tab state updates", async () => {
    const { result } = renderHook(() => useStandaloneNoteTab("session-1"));

    await waitFor(() => {
      expect(useTabs.getState().tabs).toHaveLength(1);
    });

    const tab = useTabs.getState().tabs[0];
    expect(tab).toMatchObject({
      active: true,
      id: "session-1",
      type: "sessions",
    });

    act(() => {
      useTabs.getState().updateSessionTabState(tab, {
        autoStart: null,
        view: { type: "raw" },
      });
    });

    expect(result.current.state.view).toEqual({ type: "raw" });
  });

  it("attaches the standalone note to live session events", () => {
    renderHook(() => useAttachStandaloneNoteToLiveSession("session-1"));

    expect(mocks.attachLiveSession).toHaveBeenCalledWith("session-1");
  });

  it("reattaches after standalone live session events are removed", () => {
    const { rerender } = renderHook(() =>
      useAttachStandaloneNoteToLiveSession("session-1"),
    );
    expect(mocks.attachLiveSession).toHaveBeenCalledTimes(1);

    mocks.listenerState.live.eventUnlistenersBySession = {
      "session-1": [vi.fn()],
    };
    rerender();
    expect(mocks.attachLiveSession).toHaveBeenCalledTimes(1);

    mocks.listenerState.live.eventUnlistenersBySession = {};
    rerender();
    expect(mocks.attachLiveSession).toHaveBeenCalledTimes(2);
  });
});

function dispatchKeyDown(key: string, target: EventTarget = window) {
  const event = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    key,
  });
  target.dispatchEvent(event);
  return event;
}
