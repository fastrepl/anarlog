import { act, renderHook } from "@testing-library/react";
import { isValidElement } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  hotkeys: new Map<
    string,
    {
      handler: () => void;
      options?: {
        enabled?: boolean;
      };
    }
  >(),
}));

vi.mock("react-hotkeys-hook", () => ({
  useHotkeys: (
    keys: string,
    handler: () => void,
    options?: {
      enabled?: boolean;
    },
  ) => {
    hoisted.hotkeys.set(keys, { handler, options });
  },
}));

vi.mock("./during-session", () => ({
  DuringSessionAccessory: () => null,
}));

vi.mock("./post-session", () => ({
  PostSessionAccessory: () => null,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: {
        requestedLiveTranscription: boolean | null;
        liveTranscriptionActive: boolean | null;
      };
    }) => unknown,
  ) =>
    selector({
      live: {
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
      },
    }),
}));

const { useShellMock } = vi.hoisted(() => ({
  useShellMock: vi.fn(),
}));

vi.mock("~/contexts/shell", () => ({
  useShell: useShellMock,
}));

import { useSessionBottomAccessory } from "./index";

describe("useSessionBottomAccessory", () => {
  beforeEach(() => {
    hoisted.hotkeys.clear();
    useShellMock.mockReturnValue({
      chat: {
        mode: "Closed",
      },
    });
  });

  it("collapses the post-session transcript panel on escape", () => {
    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "inactive",
        audioUrl: "file:///session.wav",
        hasTranscript: true,
      }),
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: false,
    });
    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(false);

    const toggle = result.current.bottomBorderHandle;
    expect(isValidElement<{ onToggle: () => void }>(toggle)).toBe(true);
    if (!isValidElement<{ onToggle: () => void }>(toggle)) {
      return;
    }

    act(() => {
      toggle.props.onToggle();
    });

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: true,
    });
    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(true);

    act(() => {
      hoisted.hotkeys.get("esc")?.handler();
    });

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: false,
    });
    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(false);
  });

  it("defers transcript escape handling while chat is open", () => {
    useShellMock.mockReturnValue({
      chat: {
        mode: "RightPanelOpen",
      },
    });

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "inactive",
        audioUrl: "file:///session.wav",
        hasTranscript: true,
      }),
    );

    const toggle = result.current.bottomBorderHandle;
    expect(isValidElement<{ onToggle: () => void }>(toggle)).toBe(true);
    if (!isValidElement<{ onToggle: () => void }>(toggle)) {
      return;
    }

    act(() => {
      toggle.props.onToggle();
    });

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: true,
    });
    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(false);
  });
});
