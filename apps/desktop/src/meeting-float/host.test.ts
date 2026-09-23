import { describe, expect, it, vi } from "vitest";

import type {
  LiveTranscriptSegment,
  RenderedTranscriptSegment,
} from "@anlg/plugin-transcription";

const transcriptMocks = vi.hoisted(() => ({
  renderTranscriptSegments: vi.fn(),
}));

vi.mock("@anlg/plugin-transcription", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("@anlg/plugin-transcription")>();
  return {
    ...actual,
    commands: {
      ...actual.commands,
      renderTranscriptSegments: transcriptMocks.renderTranscriptSegments,
    },
  };
});

import { createMeetingFloatLabelContext, type MeetingFloatData } from "./hooks";
import {
  createFloatingSpeakerResolver,
  getCurrentFloatingBarColorScheme,
  getFloatingRouteState,
  getFloatingTranscriptBubbles,
  haveFloatingRouteInputsChanged,
  shouldShowFloatingLiveCaptionToggle,
} from "./host";

import { createListenerStore } from "~/store/zustand/listener";
import { LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT } from "~/store/zustand/listener/transcript";
import { type RenderLabelContext, SegmentKeyUtils } from "~/stt/live-segment";

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

  it("shows reconnecting only during a connection attempt", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          loadingPhase: "connecting",
        }),
      )?.status,
    ).toBe("reconnecting");
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          loadingPhase: "connecting",
          lastError: "microphone unavailable",
          lastErrorIsAudioRelated: true,
        }),
      )?.status,
    ).toBe("error");
  });

  it("keeps recording status while live transcription degrades and retries", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          degraded: { type: "connection_timeout" },
        }),
      )?.status,
    ).toBe("recording");
  });

  it("returns error status when live transcription fails permanently", () => {
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          degraded: { type: "authentication_failed", provider: "deepgram" },
        }),
      )?.status,
    ).toBe("error");
    expect(
      getFloatingRouteState(
        createListenerState({
          status: "active",
          sessionId: "session-1",
          degraded: {
            type: "provider_configuration",
            provider: "deepgram",
            message: "invalid model",
          },
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

  it("keeps only the recent bounded transcript window", () => {
    const segmentCount = LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT + 5;
    const bubbles = getFloatingTranscriptBubbles(
      Array.from({ length: segmentCount }, (_, index) =>
        createSegment({
          id: `segment-${index}`,
          key: {
            channel: "RemoteParty",
            speaker_index: 0,
            speaker_human_id: null,
          },
          start_ms: index * 100,
          text: `segment ${index}`,
          words: [{ text: `segment ${index}` }],
        }),
      ),
    );

    expect(bubbles).toHaveLength(LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT);
    expect(bubbles[0]?.id).toBe("segment-5");
    expect(bubbles[bubbles.length - 1]?.id).toBe(`segment-${segmentCount - 1}`);
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

  it("labels remote bubbles as the unique other participant", () => {
    const ctx: RenderLabelContext = {
      getSelfHumanId: () => "self",
      getHumanName: (id) => (id === "remote" ? "Artem" : undefined),
      getParticipantHumanIds: () => ["self", "remote"],
    };
    const bubbles = getFloatingTranscriptBubbles(
      [
        createSegment({
          id: "remote",
          key: {
            channel: "RemoteParty",
            speaker_index: 0,
            speaker_human_id: null,
          },
          start_ms: 0,
          text: "hello",
          words: [{ text: "hello" }],
        }),
      ],
      ctx,
    );

    expect(bubbles[0]?.speakerLabel).toBe("Artem");
  });

  it("labels assigned direct-mic bubbles as self", () => {
    const ctx: RenderLabelContext = {
      getSelfHumanId: () => "self",
      getHumanName: (id) => (id === "participant-1" ? "Artem" : undefined),
      getParticipantHumanIds: () => ["self", "participant-1"],
    };
    const bubbles = getFloatingTranscriptBubbles(
      [
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
      ],
      ctx,
    );

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

  it("finds non-adjacent overlaps without rescanning the transcript", () => {
    const bubbles = getFloatingTranscriptBubbles([
      createSegment({
        id: "long-you",
        key: {
          channel: "DirectMic",
          speaker_index: null,
          speaker_human_id: null,
        },
        start_ms: 0,
        end_ms: 2000,
        text: "long local segment",
        words: [{ text: "long local segment" }],
      }),
      createSegment({
        id: "short-you",
        key: {
          channel: "DirectMic",
          speaker_index: null,
          speaker_human_id: null,
        },
        start_ms: 100,
        end_ms: 500,
        text: "short local segment",
        words: [{ text: "short local segment" }],
      }),
      createSegment({
        id: "remote",
        key: {
          channel: "RemoteParty",
          speaker_index: 0,
          speaker_human_id: null,
        },
        start_ms: 1000,
        end_ms: 1500,
        text: "remote segment",
        words: [{ text: "remote segment" }],
      }),
    ]);

    expect(bubbles).toMatchObject([
      { id: "long-you", overlapsNext: true },
      { id: "short-you", overlapsNext: false },
      { id: "remote", overlapsPrevious: true },
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

describe("shouldShowFloatingLiveCaptionToggle", () => {
  it("shows for active live transcription", () => {
    expect(
      shouldShowFloatingLiveCaptionToggle({
        provider: "anarlog",
        model: "cloud",
        liveTranscriptionActive: true,
      }),
    ).toBe(true);
  });

  it("shows for local realtime transcription", () => {
    expect(
      shouldShowFloatingLiveCaptionToggle({
        provider: "anarlog",
        model: "soniqo-parakeet-streaming",
        liveTranscriptionActive: true,
      }),
    ).toBe(true);
  });

  it("hides before live transcription is active", () => {
    expect(
      shouldShowFloatingLiveCaptionToggle({
        provider: "anarlog",
        model: "cloud",
        liveTranscriptionActive: false,
      }),
    ).toBe(false);
  });
});

describe("floating route refresh", () => {
  it("refreshes when retry state changes without new audio", () => {
    const previous = createListenerState({
      status: "active",
      sessionId: "session-1",
    });
    const retrying = {
      ...previous,
      live: { ...previous.live, loadingPhase: "connecting" as const },
    };
    expect(haveFloatingRouteInputsChanged(retrying, previous)).toBe(true);
    expect(haveFloatingRouteInputsChanged(previous, retrying)).toBe(true);
    expect(haveFloatingRouteInputsChanged(previous, previous)).toBe(false);
    const audioFailure = {
      ...retrying,
      live: { ...retrying.live, lastErrorIsAudioRelated: true },
    };
    expect(haveFloatingRouteInputsChanged(audioFailure, retrying)).toBe(true);
    expect(haveFloatingRouteInputsChanged(retrying, audioFailure)).toBe(true);
  });

  it("refreshes when the degraded type changes without new audio", () => {
    const previous = createListenerState({
      status: "active",
      sessionId: "session-1",
    });
    const timeout = {
      ...previous,
      live: {
        ...previous.live,
        degraded: { type: "connection_timeout" as const },
      },
    };
    const authFailed = {
      ...previous,
      live: {
        ...previous.live,
        degraded: {
          type: "authentication_failed" as const,
          provider: "deepgram",
        },
      },
    };
    expect(haveFloatingRouteInputsChanged(timeout, previous)).toBe(true);
    expect(haveFloatingRouteInputsChanged(authFailed, timeout)).toBe(true);
    expect(haveFloatingRouteInputsChanged(timeout, authFailed)).toBe(true);
    expect(haveFloatingRouteInputsChanged(timeout, timeout)).toBe(false);
  });
});

describe("createFloatingSpeakerResolver", () => {
  const speakerContext = {
    intervals: [
      {
        start_ms: 0,
        end_ms: 60_000,
        active_call: true,
        calendar_call: false,
        mic_isolated: null,
        shared_microphone: false,
        title: "",
        self_names: [],
        participants: [{ human_id: "human-remote", name: "Artem" }],
      },
    ],
  };

  const floatData = (context = speakerContext): MeetingFloatData => ({
    sessions: {
      "session-1": {
        title: "Planning",
        ownerUserId: "human-self",
        participantHumanIds: ["human-remote"],
        speakerContext: context,
        startedAtMs: 1_000,
      },
    },
    humanNames: { "human-remote": "Artem" },
  });

  const remoteSegment = () =>
    createSegment({
      id: "seg-remote",
      key: {
        channel: "RemoteParty",
        speaker_index: 1,
        speaker_human_id: null,
      },
      start_ms: 0,
      text: "hello",
      words: [{ text: "hello" }],
    });

  const renderedRemoteSegment = (
    humanId: string | null,
  ): RenderedTranscriptSegment => ({
    id: "seg-remote",
    key: {
      channel: "RemoteParty",
      speaker_index: 1,
      speaker_human_id: null,
    },
    start_ms: 0,
    end_ms: 100,
    text: "hello",
    speaker_label: "Artem",
    provisional_speaker: {
      name: "Artem",
      human_id: humanId,
      reason: "sole_remote_participant",
    },
    words: [],
  });

  const deferred = <T>() => {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>((res) => {
      resolve = res;
    });
    return { promise, resolve };
  };

  it("labels bubbles with the resolver result instead of Speaker N", async () => {
    transcriptMocks.renderTranscriptSegments.mockResolvedValue({
      status: "ok",
      data: [renderedRemoteSegment("human-remote")],
    });
    const labels = new Map<string, { label: string; humanId?: string }>();
    const onUpdate = vi.fn();
    const resolve = createFloatingSpeakerResolver(
      labels,
      () => floatData(),
      onUpdate,
    );
    const state = createListenerStateWithSegments(
      { status: "active", sessionId: "session-1" },
      [remoteSegment()],
    );

    resolve(state);
    await vi.waitFor(() => expect(onUpdate).toHaveBeenCalled());

    const key = SegmentKeyUtils.serialize(remoteSegment().key);
    expect(labels.get(key)).toEqual({
      label: "Artem",
      humanId: "human-remote",
    });

    const bubbles = getFloatingTranscriptBubbles(
      state.liveSegments,
      createMeetingFloatLabelContext(floatData(), "session-1"),
      labels,
    );
    expect(bubbles[0]?.speakerLabel).toBe("Artem");
  });

  it("renders You when the resolved speaker is the session owner", async () => {
    transcriptMocks.renderTranscriptSegments.mockResolvedValue({
      status: "ok",
      data: [renderedRemoteSegment("human-self")],
    });
    const labels = new Map<string, { label: string; humanId?: string }>();
    const resolve = createFloatingSpeakerResolver(
      labels,
      () => floatData(),
      vi.fn(),
    );
    const state = createListenerStateWithSegments(
      { status: "active", sessionId: "session-1" },
      [remoteSegment()],
    );

    resolve(state);
    await vi.waitFor(() => expect(labels.size).toBe(1));

    const bubbles = getFloatingTranscriptBubbles(
      state.liveSegments,
      createMeetingFloatLabelContext(floatData(), "session-1"),
      labels,
    );
    expect(bubbles[0]?.speakerLabel).toBe("You");
  });

  it("ignores a stale response issued before the latest request", async () => {
    const first = deferred<{
      status: "ok";
      data: RenderedTranscriptSegment[];
    }>();
    const second = deferred<{
      status: "ok";
      data: RenderedTranscriptSegment[];
    }>();
    transcriptMocks.renderTranscriptSegments
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const labels = new Map<string, { label: string; humanId?: string }>();
    const onUpdate = vi.fn();
    const resolve = createFloatingSpeakerResolver(
      labels,
      () => floatData(),
      onUpdate,
    );

    resolve(
      createListenerStateWithSegments(
        { status: "active", sessionId: "session-1" },
        [remoteSegment()],
      ),
    );
    resolve(
      createListenerStateWithSegments(
        { status: "active", sessionId: "session-1" },
        [remoteSegment()],
      ),
    );

    first.resolve({
      status: "ok",
      data: [
        { ...renderedRemoteSegment("human-remote"), speaker_label: "Stale" },
      ],
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(labels.size).toBe(0);
    expect(onUpdate).not.toHaveBeenCalled();

    second.resolve({
      status: "ok",
      data: [renderedRemoteSegment("human-remote")],
    });
    await vi.waitFor(() => expect(labels.size).toBe(1));
    expect(onUpdate).toHaveBeenCalledTimes(1);
  });

  it("does not call the native resolver without a speaker context", () => {
    const labels = new Map<string, { label: string; humanId?: string }>();
    const resolve = createFloatingSpeakerResolver(
      labels,
      () => floatData({ intervals: [] }),
      vi.fn(),
    );

    resolve(
      createListenerStateWithSegments(
        { status: "active", sessionId: "session-1" },
        [remoteSegment()],
      ),
    );

    expect(transcriptMocks.renderTranscriptSegments).not.toHaveBeenCalled();
  });
});
