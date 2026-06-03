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
  live: {
    status: "inactive" as "inactive" | "active" | "finalizing",
    sessionId: null as string | null,
    requestedLiveTranscription: true as boolean | null,
    liveTranscriptionActive: true as boolean | null,
  },
  pastNotes: [] as Array<{
    sessionId: string;
    title: string;
    dateLabel: string;
    summary: string | null;
    isGenerating: boolean;
  }>,
  generateMissingPastNotes: vi.fn(),
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

vi.mock("./past-notes", () => ({
  usePastSessionNotes: () => ({
    notes: hoisted.pastNotes,
    hasPastNotes: hoisted.pastNotes.length > 0,
    isGenerating: false,
    canGenerate: true,
    generateMissing: hoisted.generateMissingPastNotes,
  }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: {
        status: "inactive" | "active" | "finalizing";
        sessionId: string | null;
        requestedLiveTranscription: boolean | null;
        liveTranscriptionActive: boolean | null;
      };
    }) => unknown,
  ) =>
    selector({
      live: hoisted.live,
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
    hoisted.live.status = "inactive";
    hoisted.live.sessionId = null;
    hoisted.live.requestedLiveTranscription = true;
    hoisted.live.liveTranscriptionActive = true;
    hoisted.pastNotes = [];
    hoisted.generateMissingPastNotes.mockClear();
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
        mode: "FloatingOpen",
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

  it("defers transcript escape handling while right panel chat is open", () => {
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

    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(false);
  });

  it("keeps the playback accessory mounted while the transcript panel is collapsed", () => {
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
    expect(result.current.bottomAccessory).not.toBeNull();
  });

  it("generates missing past note facts when the past notes tab opens", () => {
    hoisted.pastNotes = [
      {
        sessionId: "past-session",
        title: "Weekly sync",
        dateLabel: "May 28, 2026",
        summary: null,
        isGenerating: false,
      },
    ];

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "inactive",
        audioUrl: "file:///session.wav",
        hasTranscript: true,
      }),
    );

    const handle = result.current.bottomBorderHandle;
    expect(
      isValidElement<{ onSelect: (tab: "past_notes") => void }>(handle),
    ).toBe(true);
    if (!isValidElement<{ onSelect: (tab: "past_notes") => void }>(handle)) {
      return;
    }

    act(() => {
      handle.props.onSelect("past_notes");
    });

    expect(hoisted.generateMissingPastNotes).toHaveBeenCalledTimes(1);
    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: true,
    });
  });

  it("hides the bottom accessory while recording for batch transcription", () => {
    hoisted.live.requestedLiveTranscription = false;
    hoisted.live.liveTranscriptionActive = false;

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "active",
        audioUrl: null,
        hasTranscript: false,
      }),
    );

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomAccessory).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("defers local transcript controls to the global live panel for another active session", () => {
    hoisted.live.status = "active";
    hoisted.live.sessionId = "live-session";

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "inactive",
        audioUrl: "file:///session.wav",
        hasTranscript: true,
      }),
    );

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomAccessory).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("keeps batch progress visible while another session is live", () => {
    hoisted.live.status = "active";
    hoisted.live.sessionId = "live-session";

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "running_batch",
        audioUrl: "file:///session.wav",
        hasTranscript: true,
      }),
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: false,
    });
    expect(result.current.bottomAccessory).not.toBeNull();
    expect(result.current.bottomBorderHandle).not.toBeNull();
  });

  it("keeps batch progress visible while batch transcription is running", () => {
    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "running_batch",
        audioUrl: "file:///session.wav",
        hasTranscript: true,
      }),
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: false,
    });
    expect(result.current.bottomAccessory).not.toBeNull();
    expect(result.current.bottomBorderHandle).not.toBeNull();
  });

  it("keeps the transcript panel expanded when regeneration starts", () => {
    const { result, rerender } = renderHook(
      ({ sessionMode }: { sessionMode: string }) =>
        useSessionBottomAccessory({
          sessionId: "session-1",
          sessionMode,
          audioUrl: "file:///session.wav",
          hasTranscript: true,
        }),
      {
        initialProps: {
          sessionMode: "inactive",
        },
      },
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

    rerender({ sessionMode: "running_batch" });

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: true,
    });
    expect(result.current.bottomAccessory).not.toBeNull();
  });

  it("keeps the expanded live handle on neutral 50", () => {
    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "active",
        audioUrl: null,
        hasTranscript: false,
      }),
    );

    const toggle = result.current.bottomBorderHandle;
    expect(
      isValidElement<{
        expandedClassName?: string;
        isExpanded: boolean;
        label?: string;
        onToggle: () => void;
      }>(toggle),
    ).toBe(true);
    if (
      !isValidElement<{
        expandedClassName?: string;
        label?: string;
        onToggle: () => void;
      }>(toggle)
    ) {
      return;
    }

    expect(toggle.props.label).toBe("Live");
    expect(toggle.props.expandedClassName).toBe("bg-neutral-50");

    act(() => {
      toggle.props.onToggle();
    });

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "live",
      expanded: true,
    });
  });
});
