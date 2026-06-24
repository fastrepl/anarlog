import { describe, expect, it } from "vitest";

import {
  getCurrentFloatingBarColorScheme,
  getFloatingRouteState,
  getLiveCaptionRouteState,
  shouldShowFloatingLiveCaptionToggle,
} from "./host";

import { createListenerStore } from "~/store/zustand/listener";

type ListenerLiveState = ReturnType<
  ReturnType<typeof createListenerStore>["getState"]
>["live"];

function createListenerState(live: Partial<ListenerLiveState>) {
  const store = createListenerStore();
  store.setState({
    live: {
      ...store.getState().live,
      ...live,
    },
  });
  return store.getState();
}

function createListenerStateWithCaption(
  live: Partial<ListenerLiveState>,
  liveCaptionText: string,
) {
  const store = createListenerStore();
  store.setState({
    live: {
      ...store.getState().live,
      ...live,
    },
    liveCaptionText,
  });
  return store.getState();
}

describe("getFloatingRouteState", () => {
  it("returns recording status for healthy live sessions", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          amplitude: { mic: 0.6, speaker: 0.8 },
        }),
      ),
    ).toEqual({
      sessionId: "session-1",
      amplitude: 1,
      status: "recording",
      colorScheme: "dark",
      opacity: 0.78,
      liveCaptionOpacity: 0.3,
      liveCaptionWidth: 440,
      liveCaptionLineCount: 1,
      liveCaptionPosition: "topCenter",
      liveCaptionMinimized: false,
      liveCaptionToggleVisible: false,
    });
  });

  it("marks the transcript toggle visible for cloud live transcription", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          liveTranscriptionActive: true,
        }),
        {
          liveCaptionToggleVisible: true,
        },
      )?.liveCaptionToggleVisible,
    ).toBe(true);
  });

  it("returns error status when live transcription degrades", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          degraded: { type: "connection_timeout" },
        }),
      )?.status,
    ).toBe("error");
  });

  it("returns error status when the active listener reports an error", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          lastError: "microphone unavailable",
        }),
      )?.status,
    ).toBe("error");
  });

  it("hides the floating route while the session is finalizing", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "finalizing",
          sessionId: "session-1",
        }),
      ),
    ).toBeNull();
  });
});

describe("getCurrentFloatingBarColorScheme", () => {
  it("uses the applied document theme", () => {
    document.documentElement.classList.remove("dark");
    expect(getCurrentFloatingBarColorScheme()).toBe("light");

    document.documentElement.classList.add("dark");
    expect(getCurrentFloatingBarColorScheme()).toBe("dark");
  });
});

describe("getLiveCaptionRouteState", () => {
  it("returns live caption state for active live transcription", () => {
    expect(
      getLiveCaptionRouteState(
        createListenerStateWithCaption(
          {
            status: "active",
            sessionId: "session-1",
            liveTranscriptionActive: true,
          },
          "  we should ship this  ",
        ),
      ),
    ).toEqual({
      sessionId: "session-1",
      text: "we should ship this",
      opacity: 0.3,
      width: 440,
      lineCount: 1,
      position: "topCenter",
      minimized: false,
    });
  });

  it("hides captions when the floating bar toggle has hidden them", () => {
    expect(
      getLiveCaptionRouteState(
        createListenerStateWithCaption(
          {
            status: "active",
            sessionId: "session-1",
            liveTranscriptionActive: true,
          },
          " ",
        ),
        {
          floatingBarOpacity: 0.7,
          liveCaptionOpacity: 0.66,
          liveCaptionWidth: 520,
          liveCaptionLineCount: 3,
          liveCaptionPosition: "bottomRight",
          liveCaptionMinimized: true,
        },
      ),
    ).toBeNull();
  });

  it("hides captions before live transcription is active", () => {
    expect(
      getLiveCaptionRouteState(
        createListenerStateWithCaption(
          {
            status: "active",
            sessionId: "session-1",
            liveTranscriptionActive: false,
          },
          "hello",
        ),
      ),
    ).toBeNull();
  });

  it("shows captions immediately before text arrives", () => {
    expect(
      getLiveCaptionRouteState(
        createListenerStateWithCaption(
          {
            status: "active",
            sessionId: "session-1",
            liveTranscriptionActive: true,
          },
          " ",
        ),
      ),
    ).toEqual({
      sessionId: "session-1",
      text: "",
      opacity: 0.3,
      width: 440,
      lineCount: 1,
      position: "topCenter",
      minimized: false,
    });
  });
});

describe("shouldShowFloatingLiveCaptionToggle", () => {
  it("shows for active live transcription", () => {
    expect(
      shouldShowFloatingLiveCaptionToggle({
        provider: "hyprnote",
        model: "cloud",
        liveTranscriptionActive: true,
      }),
    ).toBe(true);
  });

  it("shows for local realtime transcription", () => {
    expect(
      shouldShowFloatingLiveCaptionToggle({
        provider: "hyprnote",
        model: "soniqo-parakeet-streaming",
        liveTranscriptionActive: true,
      }),
    ).toBe(true);
  });

  it("hides before live transcription is active", () => {
    expect(
      shouldShowFloatingLiveCaptionToggle({
        provider: "hyprnote",
        model: "cloud",
        liveTranscriptionActive: false,
      }),
    ).toBe(false);
  });
});
