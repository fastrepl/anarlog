import { describe, expect, it } from "vitest";

import type { LiveTranscriptSegment } from "@hypr/plugin-transcription";

import {
  getCurrentFloatingBarColorScheme,
  getFloatingRouteState,
  getFloatingTranscriptBubbles,
  getLiveCaptionDisplayText,
  getLiveCaptionRouteState,
  shouldShowFloatingLiveCaptionToggle,
} from "./host";

import { createListenerStore } from "~/store/zustand/listener";

type ListenerLiveState = ReturnType<
  ReturnType<typeof createListenerStore>["getState"]
>["live"];
type SegmentWord = LiveTranscriptSegment["words"][number];

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

function createListenerStateWithSegments(
  live: Partial<ListenerLiveState>,
  liveSegments: LiveTranscriptSegment[],
) {
  const store = createListenerStore();
  store.setState({
    live: {
      ...store.getState().live,
      ...live,
    },
    liveSegments,
  });
  return store.getState();
}

function createSegment(
  segment: Omit<LiveTranscriptSegment, "end_ms" | "words"> & {
    words: Array<Partial<SegmentWord> & Pick<SegmentWord, "text">>;
    end_ms?: number;
  },
): LiveTranscriptSegment {
  return {
    ...segment,
    end_ms: segment.end_ms ?? segment.start_ms + 100,
    words: segment.words.map((word, index) => ({
      start_ms: word.start_ms ?? segment.start_ms + index * 10,
      end_ms: word.end_ms ?? segment.start_ms + index * 10 + 5,
      channel: word.channel ?? segment.key.channel,
      is_final: word.is_final ?? true,
      text: word.text,
      id: word.id,
    })),
  };
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
      title: "Live transcript",
      amplitude: 1,
      status: "recording",
      colorScheme: "dark",
      opacity: 0.78,
      liveCaptionOpacity: 0.3,
      liveCaptionWidth: 440,
      liveCaptionLineCount: 1,
      liveCaptionPosition: "topCenter",
      liveCaptionMinimized: true,
      liveCaptionToggleVisible: false,
      transcriptBubbles: [],
    });
  });

  it("uses the session title when provided", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
        }),
        { sessionTitle: "  Weekly team sync  " },
      )?.title,
    ).toBe("Weekly team sync");
  });

  it("builds transcript bubbles from speaker segments", () => {
    const segments = [
      createSegment({
        id: "remote-1",
        key: {
          channel: "RemoteParty",
          speaker_index: 1,
          speaker_human_id: null,
        },
        start_ms: 200,
        text: "being bingo",
        words: [{ text: "being", is_final: false }, { text: "bingo" }],
      }),
      createSegment({
        id: "mic-1",
        key: {
          channel: "DirectMic",
          speaker_index: null,
          speaker_human_id: null,
        },
        start_ms: 100,
        text: "summary yep",
        words: [{ text: "summary" }, { text: "yep" }, { text: "." }],
      }),
    ];

    expect(
      getFloatingRouteState(
        createListenerStateWithSegments(
          {
            status: "active",
            sessionId: "session-1",
            liveTranscriptionActive: true,
          },
          segments,
        ),
        { liveCaptionToggleVisible: true },
      )?.transcriptBubbles,
    ).toEqual([
      {
        id: "mic-1",
        speakerLabel: "You",
        text: "summary yep.",
        isSelf: true,
        isFinal: true,
        startMs: 100,
        endMs: 200,
        overlapsPrevious: false,
        overlapsNext: false,
      },
      {
        id: "remote-1",
        speakerLabel: "Speaker 2",
        text: "being bingo",
        isSelf: false,
        isFinal: false,
        startMs: 200,
        endMs: 300,
        overlapsPrevious: false,
        overlapsNext: false,
      },
    ]);
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

describe("getFloatingTranscriptBubbles", () => {
  it("keeps all transcript bubbles in chronological order", () => {
    const bubbles = getFloatingTranscriptBubbles(
      Array.from({ length: 8 }, (_, index) =>
        createSegment({
          id: `segment-${index}`,
          key: {
            channel: "RemoteParty",
            speaker_index: index % 2,
            speaker_human_id: null,
          },
          start_ms: index,
          text: `segment ${index}`,
          words: [{ text: `segment ${index}` }],
        }),
      ),
    );

    expect(bubbles.map((bubble) => bubble.id)).toEqual([
      "segment-0",
      "segment-1",
      "segment-2",
      "segment-3",
      "segment-4",
      "segment-5",
      "segment-6",
      "segment-7",
    ]);
  });

  it("labels diarized direct-mic bubbles as self", () => {
    const bubbles = getFloatingTranscriptBubbles([
      createSegment({
        id: "local-mic",
        key: {
          channel: "DirectMic",
          speaker_index: 2,
          speaker_human_id: null,
        },
        start_ms: 0,
        text: "hello",
        words: [{ text: "hello" }],
      }),
    ]);

    expect(bubbles).toEqual([
      {
        id: "local-mic",
        speakerLabel: "You",
        text: "hello",
        isSelf: true,
        isFinal: true,
        startMs: 0,
        endMs: 100,
        overlapsPrevious: false,
        overlapsNext: false,
      },
    ]);
  });

  it("labels assigned direct-mic bubbles as self", () => {
    const bubbles = getFloatingTranscriptBubbles([
      createSegment({
        id: "assigned-mic",
        key: {
          channel: "DirectMic",
          speaker_index: 1,
          speaker_human_id: "participant-1",
        },
        start_ms: 0,
        text: "hello",
        words: [{ text: "hello" }],
      }),
    ]);

    expect(bubbles).toEqual([
      {
        id: "assigned-mic",
        speakerLabel: "You",
        text: "hello",
        isSelf: true,
        isFinal: true,
        startMs: 0,
        endMs: 100,
        overlapsPrevious: false,
        overlapsNext: false,
      },
    ]);
  });

  it("marks bubbles that overlap different speakers", () => {
    const bubbles = getFloatingTranscriptBubbles([
      createSegment({
        id: "you",
        key: {
          channel: "DirectMic",
          speaker_index: null,
          speaker_human_id: null,
        },
        start_ms: 100,
        end_ms: 900,
        text: "how it changes",
        words: [{ text: "how" }, { text: "it" }, { text: "changes" }],
      }),
      createSegment({
        id: "speaker",
        key: {
          channel: "RemoteParty",
          speaker_index: 0,
          speaker_human_id: null,
        },
        start_ms: 500,
        end_ms: 1100,
        text: "ah how it changes",
        words: [{ text: "ah" }, { text: "how" }, { text: "it" }],
      }),
    ]);

    expect(bubbles).toMatchObject([
      {
        id: "you",
        overlapsPrevious: false,
        overlapsNext: true,
      },
      {
        id: "speaker",
        overlapsPrevious: true,
        overlapsNext: false,
      },
    ]);
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
  it("hides live captions by default", () => {
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
    ).toBeNull();
  });

  it("returns live caption state for active live transcription when expanded", () => {
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
        {
          floatingBarOpacity: 0.78,
          liveCaptionOpacity: 0.3,
          liveCaptionWidth: 440,
          liveCaptionLineCount: 1,
          liveCaptionPosition: "topCenter",
          liveCaptionMinimized: false,
        },
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

  it("hides captions when the live caption is hidden from the floating bar", () => {
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

  it("keeps the minimized caption restore control visible without text", () => {
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
        {
          floatingBarOpacity: 0.78,
          liveCaptionOpacity: 0.3,
          liveCaptionWidth: 440,
          liveCaptionLineCount: 1,
          liveCaptionPosition: "topCenter",
          liveCaptionMinimized: false,
        },
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

describe("getLiveCaptionDisplayText", () => {
  it("shows a rolling recent caption window", () => {
    const text =
      "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega";

    const singleLine = getLiveCaptionDisplayText(text, {
      liveCaptionWidth: 260,
      liveCaptionLineCount: 1,
    });
    const expanded = getLiveCaptionDisplayText(text, {
      liveCaptionWidth: 260,
      liveCaptionLineCount: 3,
    });

    expect(singleLine).toMatch(/^\.\.\. /);
    expect(expanded).toMatch(/^\.\.\. /);
    expect(singleLine.endsWith("omega")).toBe(true);
    expect(expanded.endsWith("omega")).toBe(true);
    expect(expanded.length).toBeGreaterThan(singleLine.length);
    expect(expanded.endsWith(singleLine.replace(/^\.\.\. /, ""))).toBe(true);
  });

  it("keeps short captions unchanged", () => {
    expect(
      getLiveCaptionDisplayText("  hello   there  ", {
        liveCaptionWidth: 260,
        liveCaptionLineCount: 1,
      }),
    ).toBe("hello there");
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
