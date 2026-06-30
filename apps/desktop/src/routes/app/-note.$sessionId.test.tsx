import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { resetTabsStore } from "~/store/zustand/tabs/test-utils";

const mocks = vi.hoisted(() => ({
  attachLiveSession: vi.fn(),
  close: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: mocks.close,
  }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      attachLiveSession: typeof mocks.attachLiveSession;
    }) => unknown,
  ) => selector({ attachLiveSession: mocks.attachLiveSession }),
}));

import {
  useAttachStandaloneNoteToLiveSession,
  useCloseStandaloneNoteWindowOnEscape,
  useStandaloneNoteTab,
} from "./note.$sessionId";

import { useTabs } from "~/store/zustand/tabs";

describe("standalone note window route", () => {
  beforeEach(() => {
    mocks.attachLiveSession.mockClear();
    mocks.close.mockClear();
    resetTabsStore();
  });

  afterEach(() => {
    cleanup();
  });

  it("closes the standalone note window on escape", () => {
    renderHook(() => useCloseStandaloneNoteWindowOnEscape());

    const event = dispatchKeyDown("Escape");

    expect(event.defaultPrevented).toBe(true);
    expect(mocks.close).toHaveBeenCalledTimes(1);
  });

  it("ignores other keys", () => {
    renderHook(() => useCloseStandaloneNoteWindowOnEscape());

    const event = dispatchKeyDown("Enter");

    expect(event.defaultPrevented).toBe(false);
    expect(mocks.close).not.toHaveBeenCalled();
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
});

function dispatchKeyDown(key: string) {
  const event = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    key,
  });
  window.dispatchEvent(event);
  return event;
}
