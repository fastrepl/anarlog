import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { getSessionKeywords } from "./useKeywords";
import { getPostCaptureAction } from "./useStartListening";
import { useStartListening } from "./useStartListening";

const {
  queueAutoEnhanceMock,
  queueAutoEnhanceIfSummaryEmptyMock,
  resetEnhanceTasksMock,
  startMock,
  runBatchMock,
  useListenerMock,
  useSessionMock,
  useSessionHasTranscriptMock,
  useSessionParticipantHumanIdsMock,
  createLiveTranscriptMock,
  applyLiveTranscriptDeltaToDatabaseMock,
  softDeleteTranscriptMock,
  useConfigValueMock,
  useSTTConnectionMock,
  isSupportedLanguagesLiveMock,
  leftSidebarExpanded,
  setLeftSidebarExpandedMock,
  deleteProcessedAudioForRetentionMock,
} = vi.hoisted(() => ({
  queueAutoEnhanceMock: vi.fn(),
  queueAutoEnhanceIfSummaryEmptyMock: vi.fn(),
  resetEnhanceTasksMock: vi.fn(),
  startMock: vi.fn(),
  runBatchMock: vi.fn(),
  useListenerMock: vi.fn(),
  useSessionMock: vi.fn(),
  useSessionHasTranscriptMock: vi.fn(),
  useSessionParticipantHumanIdsMock: vi.fn(),
  createLiveTranscriptMock: vi.fn(),
  applyLiveTranscriptDeltaToDatabaseMock: vi.fn(),
  softDeleteTranscriptMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  useSTTConnectionMock: vi.fn(),
  isSupportedLanguagesLiveMock: vi.fn(),
  leftSidebarExpanded: { value: true },
  setLeftSidebarExpandedMock: vi.fn(),
  deleteProcessedAudioForRetentionMock: vi.fn(),
}));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: {
    isSupportedLanguagesLive: isSupportedLanguagesLiveMock,
  },
}));

vi.mock("./contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("./useKeywords", () => ({
  getSessionKeywords: vi.fn(async () => []),
  useKeywords: vi.fn(() => []),
}));

vi.mock("./useRunBatch", () => ({
  STOPPED_TRANSCRIPTION_ERROR_MESSAGE: "Transcription stopped.",
  canRunBatchTranscription: vi.fn(() => true),
  isStoppedTranscriptionError: vi.fn(
    (error: unknown) =>
      (error instanceof Error ? error.message : String(error)) ===
      "Transcription stopped.",
  ),
  useRunBatch: vi.fn(() => runBatchMock),
}));

vi.mock("./useSTTConnection", () => ({
  useSTTConnection: useSTTConnectionMock,
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: vi.fn(() => ({
    queueAutoEnhance: queueAutoEnhanceMock,
    queueAutoEnhanceIfSummaryEmpty: queueAutoEnhanceIfSummaryEmptyMock,
    resetEnhanceTasks: resetEnhanceTasksMock,
  })),
}));

vi.mock("~/services/audio-retention", () => ({
  deleteProcessedAudioForRetention: deleteProcessedAudioForRetentionMock,
  normalizeAudioRetention: (value: unknown) =>
    typeof value === "string" ? value : "forever",
}));

vi.mock("~/contexts/shell", () => ({
  useShell: vi.fn(() => ({
    leftsidebar: {
      expanded: leftSidebarExpanded.value,
      setExpanded: setLeftSidebarExpandedMock,
    },
  })),
}));

vi.mock("~/session/utils", () => ({
  getSessionEvent: vi.fn(() => null),
}));

vi.mock("~/session/queries", () => ({
  useSession: useSessionMock,
  useSessionHasTranscript: useSessionHasTranscriptMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/shared/utils", () => ({
  id: vi.fn(() => "generated-id"),
}));

vi.mock("~/stt/queries", () => ({
  applyLiveTranscriptDeltaToDatabase: applyLiveTranscriptDeltaToDatabaseMock,
  createLiveTranscript: createLiveTranscriptMock,
  softDeleteTranscript: softDeleteTranscriptMock,
  useSessionParticipantHumanIds: useSessionParticipantHumanIdsMock,
}));

describe("getPostCaptureAction", () => {
  test("runs batch then enhance after record-only capture finishes when audio is available", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: false,
        },
        true,
      ),
    ).toBe("batch_then_enhance");
  });

  test("enhances immediately when live transcription already completed during recording", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: true,
        },
        true,
      ),
    ).toBe("enhance_only");
  });

  test("does nothing when batch fallback is needed but no transcription connection is available", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: false,
        },
        false,
      ),
    ).toBe("none");
  });

  test("does nothing when capture finishes without a saved audio path", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: null,
          liveTranscriptionActive: false,
        },
        true,
      ),
    ).toBe("none");
  });
});

describe("useStartListening", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    useListenerMock.mockImplementation((selector) =>
      selector({
        start: startMock,
      }),
    );
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "Existing memo",
    });
    useSessionHasTranscriptMock.mockReturnValue(false);
    useSessionParticipantHumanIdsMock.mockReturnValue([]);
    createLiveTranscriptMock.mockResolvedValue(undefined);
    applyLiveTranscriptDeltaToDatabaseMock.mockResolvedValue(undefined);
    softDeleteTranscriptMock.mockResolvedValue(undefined);
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "en" : [],
    );
    leftSidebarExpanded.value = true;
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "am-test",
        baseUrl: "http://localhost:8080",
        apiKey: "",
      },
    });
    startMock.mockResolvedValue(true);
    runBatchMock.mockResolvedValue(undefined);
    isSupportedLanguagesLiveMock.mockResolvedValue({
      status: "ok",
      data: true,
    });
  });

  test("collapses the left sidebar after listening starts", async () => {
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(setLeftSidebarExpandedMock).toHaveBeenCalledWith(false);
  });

  test("sets the left sidebar collapsed after listening starts even if render state is stale", async () => {
    leftSidebarExpanded.value = false;

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(setLeftSidebarExpandedMock).toHaveBeenCalledWith(false);
  });

  test("keeps the left sidebar state when listening fails to start", async () => {
    startMock.mockResolvedValue(false);

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(setLeftSidebarExpandedMock).not.toHaveBeenCalled();
  });

  test("reads keywords from the same pre-start snapshot as the transcript memo", async () => {
    const calls: string[] = [];
    vi.mocked(getSessionKeywords).mockImplementation(async () => {
      calls.push("keywords");
      return ["launch"];
    });
    startMock.mockImplementation(async () => {
      calls.push("start");
      return true;
    });

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(calls).toEqual(["keywords", "start"]);
    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      keywords: ["launch"],
    });
  });

  test("runs batch transcription after record-only capture stops", async () => {
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    expect(onStopped).toBeTypeOf("function");

    await act(async () => {
      await onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
      });
    });

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledWith(
      "forever",
      "session-1",
    );
  });

  test("cleans up processed audio after live capture stops", async () => {
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;

    await act(async () => {
      await onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
      });
    });

    expect(runBatchMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledWith(
      "forever",
      "session-1",
    );
  });

  test("regenerates the summary after resumed live capture writes transcript", async () => {
    let resolveTranscriptWrite: (() => void) | undefined;
    createLiveTranscriptMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveTranscriptWrite = resolve;
        }),
    );
    useSessionHasTranscriptMock.mockReturnValue(true);

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const handlePersist = startMock.mock.calls[0]?.[1]?.handlePersist;
    expect(handlePersist).toBeTypeOf("function");

    act(() => {
      handlePersist?.({
        new_words: [
          {
            id: "new-word",
            text: "new",
            start_ms: 100,
            end_ms: 200,
            channel: 0,
          },
        ],
        replaced_ids: [],
        partials: [],
      });
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    const stopped = onStopped?.("session-1", {
      durationSeconds: 42,
      audioPath: "/tmp/session.wav",
      requestedLiveTranscription: true,
      liveTranscriptionActive: true,
    });

    expect(resetEnhanceTasksMock).not.toHaveBeenCalled();
    resolveTranscriptWrite?.();
    await act(async () => await stopped);

    expect(createLiveTranscriptMock).toHaveBeenCalledTimes(1);
    expect(resetEnhanceTasksMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
  });

  test("regenerates the summary after resumed batch capture completes", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;

    await act(async () => {
      await onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
      });
    });

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    expect(resetEnhanceTasksMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
  });

  test("forces batch transcription for batch-only local models with realtime stored", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "soniqo-qwen3-small",
        baseUrl: "http://localhost:8080",
        apiKey: "",
      },
    });

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      transcription_mode: "batch",
    });
  });

  test("uses live transcription for realtime local models", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "soniqo-parakeet-streaming",
        baseUrl: "http://localhost:8080",
        apiKey: "",
      },
    });

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      transcription_mode: "live",
    });
  });

  test("keeps supported non-English realtime local models live", async () => {
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "de" : ["en"],
    );
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "soniqo-parakeet-streaming",
        baseUrl: "http://localhost:8080",
        apiKey: "",
      },
    });

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      languages: ["de"],
      transcription_mode: "live",
    });
  });

  test("keeps realtime local transcription live by filtering unsupported extra spoken languages", async () => {
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "en" : ["ko"],
    );
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "soniqo-parakeet-streaming",
        baseUrl: "http://localhost:8080",
        apiKey: "",
      },
    });

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      languages: ["en"],
      transcription_mode: "live",
    });
  });

  test("uses the main language for Deepgram live capture when extras are unsupported", async () => {
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "en" : ["ko"],
    );
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "deepgram",
        model: "nova-3-general",
        baseUrl: "https://api.deepgram.com/v1/listen",
        apiKey: "test-key",
      },
    });
    isSupportedLanguagesLiveMock.mockImplementation(
      (_provider, _model, languages) =>
        Promise.resolve({
          status: "ok",
          data: languages.length === 1 && languages[0] === "en",
        }),
    );

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      languages: ["en"],
      transcription_mode: undefined,
    });
  });
});
