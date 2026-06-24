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
  regenerateInsights: vi.fn(),
}));

const lingui = vi.hoisted(() => {
  type LinguiDescriptor = {
    message?: string;
    values?: Record<string, unknown>;
  };
  const isDescriptor = (value: unknown): value is LinguiDescriptor =>
    Boolean(value) && typeof value === "object" && !Array.isArray(value);
  const t = (
    input: TemplateStringsArray | LinguiDescriptor | string,
    ...values: unknown[]
  ) => {
    if (typeof input === "string") {
      return input;
    }

    if (isDescriptor(input)) {
      let message = input.message ?? "";
      const replacements =
        input.values ??
        values.find(
          (value): value is Record<string, unknown> =>
            Boolean(value) &&
            typeof value === "object" &&
            !Array.isArray(value),
        );

      if (replacements) {
        for (const [key, value] of Object.entries(replacements)) {
          message = message.split(`{${key}}`).join(String(value));
        }
      }

      return message;
    }

    return Array.from(input).reduce(
      (text, part, index) => `${text}${part}${values[index] ?? ""}`,
      "",
    );
  };

  return { t };
});

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

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
}));

vi.mock("@lingui/react", () => ({
  useLingui: () => ({
    _: lingui.t,
    t: lingui.t,
  }),
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
    hoisted.regenerateInsights.mockClear();
    useShellMock.mockReturnValue({
      chat: {
        mode: "Closed",
      },
    });
  });

  it("does not show bottom playback chrome for inactive sessions with audio", () => {
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
    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(false);
  });

  it("does not register transcript escape handling while chat is open", () => {
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

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(false);
  });

  it("does not register transcript escape handling while right panel chat is open", () => {
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

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
    expect(hoisted.hotkeys.get("esc")?.options?.enabled).toBe(false);
  });

  it("hides inactive transcript-only accessory without playback or insights", () => {
    const { result } = renderHook(() =>
      useSessionBottomAccessory({
        sessionId: "session-1",
        sessionMode: "inactive",
        audioExists: false,
        hasTranscript: true,
      }),
    );

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomAccessory).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
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

    expect(hoisted.regenerateInsights).not.toHaveBeenCalled();
    expect(result.current.bottomAccessoryState).toEqual({
      mode: "transcript_only",
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

    expect(hoisted.regenerateInsights).not.toHaveBeenCalled();
    expect(result.current.bottomAccessoryState).toEqual({
      mode: "transcript_only",
      expanded: true,
    });
  });

  it("does not render a bottom transcript panel after batch transcription fails without words", () => {
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

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomAccessory).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("shows only the insights bottom tab for batch errors next to related meetings", () => {
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
      screen.queryByRole("button", { name: "Expand Transcript" }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: "Expand Insights" }),
    ).not.toBeNull();
  });

  it("does not show inactive bottom playback when the audio URL becomes ready", () => {
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

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();

    rerender({ audioUrlReady: true });

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("hides the post-session handle while audio lookup is loading without insights", () => {
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

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();

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

  it("hides batch progress from the bottom accessory while another session is live", () => {
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

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomAccessory).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("keeps batch stop control available while batch transcription is running", () => {
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
    expect(isValidElement(result.current.bottomAccessory)).toBe(true);
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("shows batch progress when regeneration starts", () => {
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

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomBorderHandle).toBeNull();

    rerender({ sessionMode: "running_batch" });

    expect(result.current.bottomAccessoryState).toEqual({
      mode: "playback",
      expanded: false,
    });
    expect(isValidElement(result.current.bottomAccessory)).toBe(true);
    expect(result.current.bottomBorderHandle).toBeNull();
  });

  it("does not show a local bottom panel while live transcription is active", () => {
    hoisted.live.status = "active";
    hoisted.live.sessionId = "session-1";

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
});
