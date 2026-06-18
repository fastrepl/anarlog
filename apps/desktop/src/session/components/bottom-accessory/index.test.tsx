import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
} from "@testing-library/react";
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
  batch: {} as Record<string, { error: string | null }>,
  generateMissingPastNotes: vi.fn(),
  regenerateInsights: vi.fn(),
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
    regenerate: vi.fn(),
    regenerateAll: hoisted.regenerateInsights,
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
      batch: Record<string, { error: string | null }>;
    }) => unknown,
  ) =>
    selector({
      live: hoisted.live,
      batch: hoisted.batch,
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
    cleanup();
    hoisted.hotkeys.clear();
    hoisted.live.status = "inactive";
    hoisted.live.sessionId = null;
    hoisted.live.requestedLiveTranscription = true;
    hoisted.live.liveTranscriptionActive = true;
    hoisted.pastNotes = [];
    hoisted.batch = {};
    hoisted.generateMissingPastNotes.mockClear();
    hoisted.regenerateInsights.mockClear();
    useShellMock.mockReturnValue({
      chat: {
        mode: "Closed",
      },
    });
  });

  it("collapses the post-session transcript panel on escape", () => {
    type TranscriptToggleProps = {
      collapsedClassName?: string;
      onToggle: () => void;
    };

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "inactive",
        audioExists: true,
        hasTranscript: true,
      }),
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: false,
    });
    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(false);

    const toggle = result.current.bottomBorderHandle;
    expect(isValidElement<TranscriptToggleProps>(toggle)).toBe(true);
    if (!isValidElement<TranscriptToggleProps>(toggle)) {
      return;
    }

    expect(toggle.props.collapsedClassName).toBe(
      "bg-card dark:bg-app-floating-chrome",
    );

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
        audioExists: true,
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
        audioExists: true,
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

  it("hides the playback accessory while the transcript panel is collapsed", () => {
    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "inactive",
        audioExists: true,
        hasTranscript: true,
      }),
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: false,
    });
    expect(result.current.bottomAccessory).toBeNull();
  });

  it("generates missing insights when the insights tab opens", () => {
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
        audioExists: true,
        hasTranscript: true,
      }),
    );

    const handle = result.current.bottomBorderHandle;
    expect(
      isValidElement<{ onSelect: (tab: "insights") => void }>(handle),
    ).toBe(true);
    if (!isValidElement<{ onSelect: (tab: "insights") => void }>(handle)) {
      return;
    }

    act(() => {
      handle.props.onSelect("insights");
    });

    expect(hoisted.generateMissingPastNotes).toHaveBeenCalledTimes(1);
    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: true,
    });
  });

  it("uses insights as the only tab when there is no transcript content", () => {
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
        audioExists: false,
        hasTranscript: false,
      }),
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "transcript_only",
      expanded: false,
    });

    render(result.current.bottomBorderHandle);

    expect(screen.queryByRole("button", { name: /Transcript/ })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Expand Insights" }));

    expect(hoisted.generateMissingPastNotes).toHaveBeenCalledTimes(1);
    expect(result.current.bottomAccessoryState).toEqual({
      mode: "transcript_only",
      expanded: true,
    });
  });

  it("keeps the transcript panel available after batch transcription fails without words", () => {
    hoisted.batch = {
      "session-1": {
        error: "batch start failed: connection refused",
      },
    };

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "inactive",
        audioExists: false,
        hasTranscript: false,
      }),
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "transcript_only",
      expanded: false,
    });
    expect(result.current.bottomBorderHandle).not.toBeNull();
  });

  it("keeps the transcript tab visible for batch errors next to related meetings", () => {
    hoisted.batch = {
      "session-1": {
        error: "batch start failed: connection refused",
      },
    };
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
        audioExists: false,
        hasTranscript: false,
      }),
    );

    render(result.current.bottomBorderHandle);

    expect(
      screen.getByRole("button", { name: "Expand Transcript" }),
    ).not.toBeNull();
    expect(
      screen.getByRole("button", { name: "Expand Insights" }),
    ).not.toBeNull();
  });

  it("keeps playback disabled until the audio URL is ready", () => {
    const { result, rerender } = renderHook(
      ({ audioUrlReady }: { audioUrlReady: boolean }) =>
        useSessionBottomAccessory({
          sessionId: "session-1",
          sessionMode: "inactive",
          audioExists: true,
          audioUrlReady,
          hasTranscript: false,
        }),
      {
        initialProps: {
          audioUrlReady: false,
        },
      },
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "transcript_only",
      expanded: false,
    });
    expect(result.current.bottomBorderHandle).not.toBeNull();

    rerender({ audioUrlReady: true });

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: false,
    });
  });

  it("keeps the post-session handle visible while audio lookup is loading", () => {
    const { result, rerender } = renderHook(
      ({ isAudioLoading }: { isAudioLoading: boolean }) =>
        useSessionBottomAccessory({
          sessionId: "session-1",
          sessionMode: "inactive",
          audioExists: false,
          audioUrlReady: false,
          isAudioLoading,
          hasTranscript: false,
        }),
      {
        initialProps: {
          isAudioLoading: true,
        },
      },
    );

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "transcript_only",
      expanded: false,
    });
    expect(result.current.bottomBorderHandle).not.toBeNull();

    rerender({ isAudioLoading: false });

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("hides the bottom accessory while recording for batch transcription", () => {
    hoisted.live.requestedLiveTranscription = false;
    hoisted.live.liveTranscriptionActive = false;

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "active",
        audioExists: false,
        hasTranscript: false,
      }),
    );

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomAccessory).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("hides the bottom accessory while finalizing", () => {
    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "finalizing",
        audioExists: false,
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
        audioExists: true,
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
        audioExists: true,
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
        audioExists: true,
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
          audioExists: true,
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

  it("uses panel chrome for the live handle", () => {
    type LiveToggleProps = {
      collapsedClassName?: string;
      expandedClassName?: string;
      isExpanded: boolean;
      label?: string;
      onToggle: () => void;
    };

    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "active",
        audioExists: false,
        hasTranscript: false,
      }),
    );

    const toggle = result.current.bottomBorderHandle;
    expect(isValidElement<LiveToggleProps>(toggle)).toBe(true);
    if (!isValidElement<LiveToggleProps>(toggle)) {
      return;
    }

    expect(toggle.props.label).toBe("Live");
    expect(toggle.props.collapsedClassName).toBe("bg-app-floating-panel");
    expect(toggle.props.expandedClassName).toBe("bg-app-floating-panel");

    act(() => {
      toggle.props.onToggle();
    });

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "live",
      expanded: true,
    });
  });
});
