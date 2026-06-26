import { cleanup, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const hoisted = vi.hoisted(() => ({
  live: {
    status: "inactive" as "inactive" | "active" | "finalizing",
    sessionId: null as string | null,
    requestedLiveTranscription: true as boolean | null,
    liveTranscriptionActive: true as boolean | null,
  },
  batch: {} as Record<string, { error: string | null }>,
}));

vi.mock("./during-session", () => ({
  DuringSessionAccessory: () => null,
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

import { useSessionBottomAccessory } from "./index";

describe("useSessionBottomAccessory", () => {
  beforeEach(() => {
    cleanup();
    hoisted.live.status = "inactive";
    hoisted.live.sessionId = null;
    hoisted.live.requestedLiveTranscription = true;
    hoisted.live.liveTranscriptionActive = true;
    hoisted.batch = {};
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

  it("does not show an inactive bottom insights panel", () => {
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

  it("does not show insights for batch errors next to related meetings", () => {
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

  it("does not show bottom chrome while batch transcription is running", () => {
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

  it("keeps the bottom accessory hidden when regeneration starts", () => {
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

    expect(result.current.bottomAccessoryState).toBeNull();
    expect(result.current.bottomAccessory).toBeNull();
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
