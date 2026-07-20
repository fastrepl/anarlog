import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { getSessionKeywords } from "./useKeywords";
import {
  getPostCaptureAction,
  sendMeetingRecordingDisclosure,
  useStartListening,
} from "./useStartListening";

import { enqueueSessionAudioOperation } from "~/session/audio-operations";

const {
  queueAutoEnhanceMock,
  queueAutoEnhanceIfSummaryEmptyMock,
  resetEnhanceTasksMock,
  startMock,
  getSessionModeMock,
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
  listMicUsingApplicationsMock,
  sendMeetingChatMessageMock,
  sonnerToastWarningMock,
  startMeetingChatCaptureMock,
  stopMeetingChatCaptureMock,
  catalogLocalSessionAudioMock,
} = vi.hoisted(() => ({
  queueAutoEnhanceMock: vi.fn(),
  queueAutoEnhanceIfSummaryEmptyMock: vi.fn(),
  resetEnhanceTasksMock: vi.fn(),
  startMock: vi.fn(),
  getSessionModeMock: vi.fn(),
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
  listMicUsingApplicationsMock: vi.fn(),
  sendMeetingChatMessageMock: vi.fn(),
  sonnerToastWarningMock: vi.fn(),
  startMeetingChatCaptureMock: vi.fn(),
  stopMeetingChatCaptureMock: vi.fn(),
  catalogLocalSessionAudioMock: vi.fn(),
}));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: {
    isSupportedLanguagesLive: isSupportedLanguagesLiveMock,
  },
}));

vi.mock("./contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    listMicUsingApplications: listMicUsingApplicationsMock,
    sendMeetingChatMessage: sendMeetingChatMessageMock,
  },
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { warning: sonnerToastWarningMock },
}));

vi.mock("./meeting-chat-capture", () => ({
  startMeetingChatCapture: startMeetingChatCaptureMock,
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

vi.mock("~/session/attachments", () => ({
  catalogLocalSessionAudio: catalogLocalSessionAudioMock,
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

let disclosureSessionSequence = 0;

function nextDisclosureSessionId() {
  disclosureSessionSequence += 1;
  return `disclosure-session-${disclosureSessionSequence}`;
}

describe("getPostCaptureAction", () => {
  test("runs batch then enhance after record-only capture finishes when audio is available", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: false,
          needsBatchRepair: false,
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
          needsBatchRepair: false,
        },
        true,
      ),
    ).toBe("enhance_only");
  });

  test("repairs the full transcript after live transcription recovered", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: true,
          needsBatchRepair: true,
        },
        true,
      ),
    ).toBe("batch_then_enhance");
  });

  test("does nothing when batch fallback is needed but no transcription connection is available", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: false,
          needsBatchRepair: false,
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
          needsBatchRepair: false,
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
        getSessionMode: getSessionModeMock,
        start: startMock,
      }),
    );
    getSessionModeMock.mockReturnValue("active");
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
    catalogLocalSessionAudioMock.mockResolvedValue(undefined);
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat" || key === "capture_meeting_chat"
          ? false
          : [],
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
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.tinyspeck.slackmacgap", name: "Slack" }],
    });
    sendMeetingChatMessageMock.mockResolvedValue({
      status: "ok",
      data: {
        sent: true,
        warnings: [],
      },
    });
    startMeetingChatCaptureMock.mockReturnValue(stopMeetingChatCaptureMock);
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
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? true
          : [],
    );

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(setLeftSidebarExpandedMock).not.toHaveBeenCalled();
    expect(sendMeetingChatMessageMock).not.toHaveBeenCalled();
    expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();
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
        needsBatchRepair: false,
      });
    });

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav");
    expect(catalogLocalSessionAudioMock).toHaveBeenCalledWith("session-1");
    expect(
      catalogLocalSessionAudioMock.mock.invocationCallOrder[0],
    ).toBeLessThan(runBatchMock.mock.invocationCallOrder[0]!);
    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledWith(
      "forever",
      "session-1",
    );
  });

  test("skips audio cataloging when capture produces no final file", async () => {
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    await act(async () => {
      await onStopped?.("session-1", {
        durationSeconds: 0,
        audioPath: null,
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      });
    });

    expect(catalogLocalSessionAudioMock).not.toHaveBeenCalled();
  });

  test("catalogs finalized audio even when transcript persistence fails", async () => {
    createLiveTranscriptMock.mockRejectedValueOnce(new Error("write failed"));
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const callbacks = startMock.mock.calls[0]?.[1];
    callbacks?.handlePersist?.({
      new_words: [
        {
          id: "word-1",
          text: "hello",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
        },
      ],
      replaced_ids: [],
      partials: [],
    });

    await act(async () => {
      await callbacks?.onStopped?.("session-1", {
        durationSeconds: 1,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });

    expect(catalogLocalSessionAudioMock).toHaveBeenCalledWith("session-1");
    expect(runBatchMock).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  test("catalogs finalized audio through the session audio queue", async () => {
    let releaseBlocker: (() => void) | undefined;
    const blocker = enqueueSessionAudioOperation(
      "session-1",
      () =>
        new Promise<void>((resolve) => {
          releaseBlocker = resolve;
        }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    const stopped = onStopped?.("session-1", {
      durationSeconds: 1,
      audioPath: "/tmp/session.wav",
      requestedLiveTranscription: true,
      liveTranscriptionActive: true,
      needsBatchRepair: false,
    });
    await Promise.resolve();
    expect(catalogLocalSessionAudioMock).not.toHaveBeenCalled();

    releaseBlocker?.();
    await blocker;
    await act(async () => await stopped);
    expect(catalogLocalSessionAudioMock).toHaveBeenCalledWith("session-1");
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
        needsBatchRepair: false,
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
      needsBatchRepair: false,
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
        needsBatchRepair: false,
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
      key === "ai_language"
        ? "de"
        : key === "consent_auto_send_chat" || key === "capture_meeting_chat"
          ? false
          : ["en"],
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
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat" || key === "capture_meeting_chat"
          ? false
          : ["ko"],
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
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat" || key === "capture_meeting_chat"
          ? false
          : ["ko"],
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

  test("does not send the recording disclosure when auto-post is disabled", async () => {
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(sendMeetingChatMessageMock).not.toHaveBeenCalled();
    expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();
  });

  test("posts the recording disclosure after listening starts when enabled", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? true
          : [],
    );
    const sessionId = nextDisclosureSessionId();

    const { result } = renderHook(() => useStartListening(sessionId));

    await act(async () => {
      await result.current();
    });

    await waitFor(() => {
      expect(sendMeetingChatMessageMock).toHaveBeenCalledWith(
        "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so",
        ["com.tinyspeck.slackmacgap"],
      );
    });
  });

  test("posts the recording disclosure once across repeated successful starts", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? true
          : [],
    );
    const sessionId = nextDisclosureSessionId();

    const { result } = renderHook(() => useStartListening(sessionId));

    await act(async () => {
      await result.current();
    });
    await waitFor(() => {
      expect(sendMeetingChatMessageMock).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      await result.current();
    });

    expect(startMock).toHaveBeenCalledTimes(2);
    expect(sendMeetingChatMessageMock).toHaveBeenCalledTimes(1);
  });

  test("shares the once-per-session disclosure guard across hook mounts", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? true
          : [],
    );
    let resolveMicApps:
      | ((value: {
          status: "ok";
          data: { id: string; name: string }[];
        }) => void)
      | undefined;
    listMicUsingApplicationsMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveMicApps = resolve;
        }),
    );
    const sessionId = nextDisclosureSessionId();
    const firstHook = renderHook(() => useStartListening(sessionId));
    const secondHook = renderHook(() => useStartListening(sessionId));

    await act(async () => {
      await firstHook.result.current();
    });
    await act(async () => {
      await secondHook.result.current();
    });

    await act(async () => {
      resolveMicApps?.({
        status: "ok",
        data: [{ id: "com.tinyspeck.slackmacgap", name: "Slack" }],
      });
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(sendMeetingChatMessageMock).toHaveBeenCalledTimes(1);
    });
    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(1);
  });

  test("retries until Slack becomes mic-active without reporting intermediate failures", async () => {
    listMicUsingApplicationsMock
      .mockResolvedValueOnce({
        status: "ok",
        data: [{ id: "us.zoom.xos", name: "zoom.us" }],
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: [{ id: "com.tinyspeck.slackmacgap", name: "Slack" }],
      });

    await expect(
      sendMeetingRecordingDisclosure({
        maxAttempts: 2,
        retryIntervalMs: 0,
      }),
    ).resolves.toEqual({ status: "sent" });

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(2);
    expect(sendMeetingChatMessageMock).toHaveBeenCalledWith(
      expect.stringContaining("https://anarlog.so"),
      ["com.tinyspeck.slackmacgap"],
    );
    expect(sonnerToastWarningMock).not.toHaveBeenCalled();
  });

  test("keeps the Slack scope when Anarlog also appears in the mic-active apps", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? true
          : [],
    );
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [
        { id: "com.hyprnote.dev", name: "Anarlog Dev" },
        { id: "com.tinyspeck.slackmacgap", name: "Slack" },
      ],
    });
    const sessionId = nextDisclosureSessionId();

    const { result } = renderHook(() => useStartListening(sessionId));

    await act(async () => {
      await result.current();
    });

    await waitFor(() => {
      expect(sendMeetingChatMessageMock).toHaveBeenCalledWith(
        expect.stringContaining("https://anarlog.so"),
        ["com.hyprnote.dev", "com.tinyspeck.slackmacgap"],
      );
    });
  });

  test("passes an ambiguous meeting scope for Rust to reject before AX mutation", async () => {
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [
        { id: "us.zoom.xos", name: "zoom.us" },
        { id: "com.tinyspeck.slackmacgap", name: "Slack" },
      ],
    });
    sendMeetingChatMessageMock.mockResolvedValue({
      status: "ok",
      data: {
        sent: false,
        warnings: ["expected exactly one recognized meeting app bundle"],
      },
    });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    await sendMeetingRecordingDisclosure({
      maxAttempts: 1,
      retryIntervalMs: 0,
    });

    expect(sendMeetingChatMessageMock).toHaveBeenCalledWith(
      expect.stringContaining("https://anarlog.so"),
      ["us.zoom.xos", "com.tinyspeck.slackmacgap"],
    );
    expect(warn).toHaveBeenCalledWith(
      "[listener] meeting disclosure was not sent",
      "expected exactly one recognized meeting app bundle",
    );
    expect(sonnerToastWarningMock).toHaveBeenCalledWith(
      "Recording started, but Anarlog could not post the meeting chat disclosure.",
      { id: "meeting-disclosure-send-failed" },
    );
    warn.mockRestore();
  });

  test("reports one terminal failure after the bounded retry window", async () => {
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "error",
      error: "audio process query failed",
    });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      sendMeetingRecordingDisclosure({
        maxAttempts: 3,
        retryIntervalMs: 0,
      }),
    ).resolves.toEqual({
      status: "notSent",
      reason: "audio process query failed",
    });

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(3);
    expect(sendMeetingChatMessageMock).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalledTimes(1);
    expect(sonnerToastWarningMock).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });

  test("cancels disclosure before chat mutation when listening stops", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? true
          : [],
    );
    let resolveMicApps:
      | ((value: {
          status: "ok";
          data: { id: string; name: string }[];
        }) => void)
      | undefined;
    listMicUsingApplicationsMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveMicApps = resolve;
        }),
    );
    const sessionId = nextDisclosureSessionId();
    const { result } = renderHook(() => useStartListening(sessionId));

    await act(async () => {
      await result.current();
    });
    await waitFor(() => {
      expect(listMicUsingApplicationsMock).toHaveBeenCalledOnce();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    await act(async () => {
      await onStopped?.(sessionId, {
        durationSeconds: 1,
        audioPath: null,
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
      });
    });

    await act(async () => {
      resolveMicApps?.({
        status: "ok",
        data: [{ id: "com.tinyspeck.slackmacgap", name: "Slack" }],
      });
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(sendMeetingChatMessageMock).not.toHaveBeenCalled();
    expect(sonnerToastWarningMock).not.toHaveBeenCalled();
  });

  test("does not overlap disclosure sends after a quick stop and restart", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? true
          : [],
    );
    let resolveSend:
      | ((value: {
          status: "ok";
          data: { sent: boolean; warnings: string[] };
        }) => void)
      | undefined;
    sendMeetingChatMessageMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveSend = resolve;
        }),
    );
    const sessionId = nextDisclosureSessionId();
    const { result } = renderHook(() => useStartListening(sessionId));

    await act(async () => {
      await result.current();
    });
    await waitFor(() => {
      expect(sendMeetingChatMessageMock).toHaveBeenCalledOnce();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    await act(async () => {
      await onStopped?.(sessionId, {
        durationSeconds: 1,
        audioPath: null,
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
      });
      await result.current();
    });

    expect(sendMeetingChatMessageMock).toHaveBeenCalledOnce();

    await act(async () => {
      resolveSend?.({ status: "ok", data: { sent: true, warnings: [] } });
      await Promise.resolve();
    });
    expect(sendMeetingChatMessageMock).toHaveBeenCalledOnce();
  });

  test("returns a typed failure and warns when disclosure mutation rejects", async () => {
    const error = new Error("IPC unavailable");
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    sendMeetingChatMessageMock.mockRejectedValueOnce(error);

    await expect(
      sendMeetingRecordingDisclosure({
        maxAttempts: 1,
        retryIntervalMs: 0,
      }),
    ).resolves.toEqual({ status: "notSent", reason: "IPC unavailable" });

    expect(warn).toHaveBeenCalledWith(
      "[listener] meeting disclosure was not sent",
      error,
    );
    expect(sonnerToastWarningMock).toHaveBeenCalledWith(
      "Recording started, but Anarlog could not post the meeting chat disclosure.",
      { id: "meeting-disclosure-send-failed" },
    );
    warn.mockRestore();
  });

  test("starts meeting chat capture with the disclosure text excluded", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? false
          : [],
    );

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    await waitFor(() => {
      expect(startMeetingChatCaptureMock).toHaveBeenCalledWith({
        sessionId: "session-1",
        excludedTexts: [
          "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so",
        ],
      });
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    await act(async () => {
      await onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: null,
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
      });
    });
    expect(stopMeetingChatCaptureMock).toHaveBeenCalledOnce();
  });

  test("starts capture discovery before a supported meeting app is active", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "consent_auto_send_chat"
          ? false
          : [],
    );
    listMicUsingApplicationsMock.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.google.Chrome", name: "Google Chrome" }],
    });

    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    await waitFor(() => {
      expect(startMeetingChatCaptureMock).toHaveBeenCalledWith({
        sessionId: "session-1",
        excludedTexts: [
          "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so",
        ],
      });
    });

    expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();
  });
});
