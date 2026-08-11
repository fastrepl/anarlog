import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  MAX_SENT_MEETING_DISCLOSURE_SESSIONS,
  startMeetingRecordingDisclosure,
} from "./meeting-disclosure";
import { getSessionKeywords } from "./useKeywords";
import {
  getPostCaptureAction,
  getPostCaptureRepairReasons,
  sendMeetingRecordingDisclosure,
  useResumeListeningLifecycle,
  useStartListening,
} from "./useStartListening";

import { enqueueSessionAudioOperation } from "~/session/audio-operations";

const {
  queueAutoEnhanceMock,
  queueAutoEnhanceIfSummaryEmptyMock,
  resetEnhanceTasksMock,
  requestAutoEnhanceMock,
  attachLiveSessionMock,
  beginCaptureRecoveryFinalizationMock,
  finishCaptureRecoveryFinalizationMock,
  canStartLiveSessionMock,
  startMock,
  getSessionModeMock,
  setBatchTranscriptionPendingMock,
  runBatchMock,
  useListenerMock,
  useSessionMock,
  useSessionHasTranscriptMock,
  useSessionParticipantHumanIdsMock,
  createLiveTranscriptMock,
  applyLiveTranscriptDeltaToDatabaseMock,
  transcriptExistsMock,
  softDeleteTranscriptMock,
  saveCaptureLifecycleMarkerMock,
  loadCaptureLifecycleMarkerMock,
  clearCaptureLifecycleMarkerMock,
  requestCaptureRecoveryMock,
  useConfigValueMock,
  useSTTConnectionMock,
  isSupportedLanguagesLiveMock,
  leftSidebarExpanded,
  setLeftSidebarExpandedMock,
  deleteProcessedAudioForRetentionMock,
  listMicUsingApplicationsMock,
  sendMeetingChatMessageMock,
  audioPathMock,
  audioSourceMetadataMock,
  sonnerToastWarningMock,
  sonnerToastErrorMock,
  sonnerToastDismissMock,
  startMeetingChatCaptureMock,
  stopMeetingChatCaptureMock,
  catalogLocalSessionAudioMock,
  markSessionAudioTranscriptionCompleteMock,
  getEnhancerServiceMock,
  requestMainAutoEnhanceMock,
  beginCloudsyncActivityMock,
  endCloudsyncActivityMock,
  flushCanonicalSessionEditorChangesMock,
  idMock,
  openNewMock,
} = vi.hoisted(() => ({
  queueAutoEnhanceMock: vi.fn(),
  queueAutoEnhanceIfSummaryEmptyMock: vi.fn(),
  resetEnhanceTasksMock: vi.fn(),
  requestAutoEnhanceMock: vi.fn(),
  attachLiveSessionMock: vi.fn(),
  beginCaptureRecoveryFinalizationMock: vi.fn(),
  finishCaptureRecoveryFinalizationMock: vi.fn(),
  canStartLiveSessionMock: vi.fn(),
  startMock: vi.fn(),
  getSessionModeMock: vi.fn(),
  setBatchTranscriptionPendingMock: vi.fn(),
  runBatchMock: vi.fn(),
  useListenerMock: vi.fn(),
  useSessionMock: vi.fn(),
  useSessionHasTranscriptMock: vi.fn(),
  useSessionParticipantHumanIdsMock: vi.fn(),
  createLiveTranscriptMock: vi.fn(),
  applyLiveTranscriptDeltaToDatabaseMock: vi.fn(),
  transcriptExistsMock: vi.fn(),
  softDeleteTranscriptMock: vi.fn(),
  saveCaptureLifecycleMarkerMock: vi.fn(),
  loadCaptureLifecycleMarkerMock: vi.fn(),
  clearCaptureLifecycleMarkerMock: vi.fn(),
  requestCaptureRecoveryMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  useSTTConnectionMock: vi.fn(),
  isSupportedLanguagesLiveMock: vi.fn(),
  leftSidebarExpanded: { value: true },
  setLeftSidebarExpandedMock: vi.fn(),
  deleteProcessedAudioForRetentionMock: vi.fn(),
  listMicUsingApplicationsMock: vi.fn(),
  sendMeetingChatMessageMock: vi.fn(),
  audioPathMock: vi.fn(),
  audioSourceMetadataMock: vi.fn(),
  sonnerToastWarningMock: vi.fn(),
  sonnerToastErrorMock: vi.fn(),
  sonnerToastDismissMock: vi.fn(),
  startMeetingChatCaptureMock: vi.fn(),
  stopMeetingChatCaptureMock: vi.fn(),
  catalogLocalSessionAudioMock: vi.fn(),
  markSessionAudioTranscriptionCompleteMock: vi.fn(),
  getEnhancerServiceMock: vi.fn(),
  requestMainAutoEnhanceMock: vi.fn(),
  beginCloudsyncActivityMock: vi.fn(),
  endCloudsyncActivityMock: vi.fn(),
  flushCanonicalSessionEditorChangesMock: vi.fn(),
  idMock: vi.fn(() => "generated-id"),
  openNewMock: vi.fn(),
}));

vi.mock("@anlg/plugin-db", () => ({
  beginCloudsyncActivity: beginCloudsyncActivityMock,
  endCloudsyncActivity: endCloudsyncActivityMock,
  execute: vi.fn(async () => []),
  executeProxy: vi.fn(async () => []),
  executeTransaction: vi.fn(async () => []),
  subscribe: vi.fn(async () => () => {}),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    isSupportedLanguagesLive: isSupportedLanguagesLiveMock,
  },
}));

vi.mock("./contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: {
    listMicUsingApplications: listMicUsingApplicationsMock,
    sendMeetingChatMessage: sendMeetingChatMessageMock,
  },
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    audioPath: audioPathMock,
    audioSourceMetadata: audioSourceMetadataMock,
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: {
    warning: sonnerToastWarningMock,
    error: sonnerToastErrorMock,
    dismiss: sonnerToastDismissMock,
  },
}));

vi.mock("~/ai/task-window-sync", () => ({
  requestMainAutoEnhance: requestMainAutoEnhanceMock,
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
  isTerminalTranscriptionError: vi.fn((error: unknown) =>
    /corrupt or unsupported|no speech|authentication failed/i.test(
      error instanceof Error ? error.message : String(error),
    ),
  ),
  useRunBatch: vi.fn(() => runBatchMock),
}));

vi.mock("./useSTTConnection", () => ({
  useSTTConnection: useSTTConnectionMock,
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: getEnhancerServiceMock,
}));

vi.mock("~/services/audio-retention", () => ({
  deleteProcessedAudioForRetention: deleteProcessedAudioForRetentionMock,
  normalizeAudioRetention: (value: unknown) =>
    typeof value === "string" ? value : "forever",
}));

vi.mock("~/session/attachments", () => ({
  catalogLocalSessionAudio: catalogLocalSessionAudioMock,
  markSessionAudioTranscriptionComplete:
    markSessionAudioTranscriptionCompleteMock,
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
  useSessionTranscriptExistence: useSessionHasTranscriptMock,
}));

vi.mock("~/session-sharing/editor-activity", () => ({
  flushCanonicalSessionEditorChanges: flushCanonicalSessionEditorChangesMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/shared/utils", () => ({
  id: idMock,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof openNewMock }) => unknown) =>
    selector({ openNew: openNewMock }),
}));

vi.mock("~/stt/capture-lifecycle-storage", () => ({
  clearCaptureLifecycleMarker: clearCaptureLifecycleMarkerMock,
  loadCaptureLifecycleMarker: loadCaptureLifecycleMarkerMock,
  saveCaptureLifecycleMarker: saveCaptureLifecycleMarkerMock,
}));

vi.mock("~/stt/capture-recovery-requests", () => ({
  requestCaptureRecovery: requestCaptureRecoveryMock,
}));

vi.mock("~/stt/queries", () => ({
  applyLiveTranscriptDeltaToDatabase: applyLiveTranscriptDeltaToDatabaseMock,
  createLiveTranscript: createLiveTranscriptMock,
  softDeleteTranscript: softDeleteTranscriptMock,
  transcriptExists: transcriptExistsMock,
  useSessionParticipantHumanIds: useSessionParticipantHumanIdsMock,
}));

let disclosureSessionSequence = 0;

function nextDisclosureSessionId() {
  disclosureSessionSequence += 1;
  return `disclosure-session-${disclosureSessionSequence}`;
}

describe("getPostCaptureAction", () => {
  test("reports every reason that requires post-stop transcript repair", () => {
    expect(
      getPostCaptureRepairReasons({
        audioPath: "/tmp/session.wav",
        liveTranscriptionActive: false,
        needsBatchRepair: true,
        transcriptWriteFailed: true,
      }),
    ).toEqual([
      "live_transcription_unavailable",
      "live_stream_incomplete",
      "transcript_persistence_failed",
    ]);
  });

  test("reports no repair reason for a complete persisted live transcript", () => {
    expect(
      getPostCaptureRepairReasons({
        audioPath: "/tmp/session.wav",
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      }),
    ).toEqual([]);
  });

  test("reports settled diarization refinement for a multi-speaker cloud transcript", () => {
    expect(
      getPostCaptureRepairReasons({
        audioPath: "/tmp/session.wav",
        liveTranscriptionActive: true,
        needsBatchRepair: false,
        refineSpeakerDiarization: true,
      }),
    ).toEqual(["settled_speaker_diarization"]);
  });

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

  test("refines a complete live transcript when settled diarization is required", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: true,
          needsBatchRepair: false,
          refineSpeakerDiarization: true,
        },
        true,
      ),
    ).toBe("batch_then_enhance");
  });

  test("keeps a complete live transcript when settled diarization cannot run", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: true,
          needsBatchRepair: false,
          refineSpeakerDiarization: true,
        },
        false,
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

  test("repairs the full transcript after a live database write fails", () => {
    expect(
      getPostCaptureAction(
        {
          audioPath: "/tmp/session.wav",
          liveTranscriptionActive: true,
          needsBatchRepair: false,
          transcriptWriteFailed: true,
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
    idMock.mockReturnValue("generated-id");

    getEnhancerServiceMock.mockImplementation(() => ({
      queueAutoEnhance: queueAutoEnhanceMock,
      queueAutoEnhanceIfSummaryEmpty: queueAutoEnhanceIfSummaryEmptyMock,
      resetEnhanceTasks: resetEnhanceTasksMock,
      requestAutoEnhance: requestAutoEnhanceMock,
    }));
    requestAutoEnhanceMock.mockImplementation(
      async (targetSessionId: string, mode: "regenerate" | "if_empty") => {
        if (mode === "regenerate") {
          await resetEnhanceTasksMock(targetSessionId);
          queueAutoEnhanceMock(targetSessionId);
          return;
        }
        await queueAutoEnhanceIfSummaryEmptyMock(targetSessionId);
      },
    );
    useListenerMock.mockImplementation((selector) =>
      selector({
        attachLiveSession: attachLiveSessionMock,
        beginCaptureRecoveryFinalization: beginCaptureRecoveryFinalizationMock,
        finishCaptureRecoveryFinalization:
          finishCaptureRecoveryFinalizationMock,
        canStartLiveSession: canStartLiveSessionMock,
        getSessionMode: getSessionModeMock,
        setBatchTranscriptionPending: setBatchTranscriptionPendingMock,
        start: startMock,
      }),
    );
    beginCaptureRecoveryFinalizationMock.mockReturnValue(true);
    canStartLiveSessionMock.mockReturnValue(true);
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
    transcriptExistsMock.mockResolvedValue(false);
    softDeleteTranscriptMock.mockResolvedValue(undefined);
    saveCaptureLifecycleMarkerMock.mockResolvedValue(undefined);
    loadCaptureLifecycleMarkerMock.mockResolvedValue(null);
    clearCaptureLifecycleMarkerMock.mockResolvedValue(undefined);
    requestCaptureRecoveryMock.mockResolvedValue(undefined);
    beginCloudsyncActivityMock.mockResolvedValue(undefined);
    endCloudsyncActivityMock.mockResolvedValue(undefined);
    flushCanonicalSessionEditorChangesMock.mockResolvedValue(undefined);
    catalogLocalSessionAudioMock.mockResolvedValue(undefined);
    markSessionAudioTranscriptionCompleteMock.mockResolvedValue(undefined);
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
        provider: "anarlog",
        model: "am-test",
        baseUrl: "http://localhost:8080",
        apiKey: "",
      },
    });
    startMock.mockResolvedValue(true);
    attachLiveSessionMock.mockResolvedValue("attached");
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
    audioPathMock.mockResolvedValue({
      status: "ok",
      data: "/tmp/existing-session.mp3",
    });
    audioSourceMetadataMock.mockResolvedValue({
      status: "ok",
      data: {
        createdAt: null,
        modifiedAt: null,
        durationMs: 60_000,
      },
    });
    startMeetingChatCaptureMock.mockReturnValue(stopMeetingChatCaptureMock);
  });

  afterEach(() => {
    vi.useRealTimers();
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
    expect(beginCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledBefore(
      endCloudsyncActivityMock,
    );
  });

  test("records without STT while offering an actionable transcription setup", async () => {
    useSTTConnectionMock.mockReturnValue({ conn: null });
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock).toHaveBeenCalledWith(
      expect.objectContaining({
        model: "",
        base_url: "",
        api_key: "",
      }),
      expect.any(Object),
    );
    expect(sonnerToastWarningMock).toHaveBeenCalledWith(
      "Live transcription is not configured",
      expect.objectContaining({
        id: "recording-without-transcription",
        description:
          "Audio is being saved. Choose a transcription provider to ensure this recording can be transcribed.",
        action: expect.objectContaining({ label: "Configure" }),
      }),
    );

    const warningCalls = sonnerToastWarningMock.mock.calls;
    const warningOptions = warningCalls[warningCalls.length - 1]?.[1];
    warningOptions?.action.onClick();

    expect(openNewMock).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "transcription" },
    });
  });

  test("does not advertise record-only capture when native recording fails", async () => {
    useSTTConnectionMock.mockReturnValue({ conn: null });
    startMock.mockResolvedValue(false);
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(sonnerToastWarningMock).not.toHaveBeenCalledWith(
      "Live transcription is not configured",
      expect.anything(),
    );
  });

  test("does not replace a capture marker while recovery blocks starting", async () => {
    canStartLiveSessionMock.mockReturnValue(false);
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(startMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
  });

  test("cleans its capture generation and releases sync when marker persistence fails", async () => {
    saveCaptureLifecycleMarkerMock.mockRejectedValueOnce(
      new Error("expected 1 affected row, got 0"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
    );
    expect(softDeleteTranscriptMock).not.toHaveBeenCalled();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledBefore(
      endCloudsyncActivityMock,
    );
    consoleError.mockRestore();
  });

  test("releases sync when failed marker cleanup also fails", async () => {
    saveCaptureLifecycleMarkerMock.mockRejectedValueOnce(
      new Error("marker write failed"),
    );
    clearCaptureLifecycleMarkerMock.mockRejectedValueOnce(
      new Error("marker cleanup failed"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
    );
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledBefore(
      endCloudsyncActivityMock,
    );
    consoleError.mockRestore();
  });

  test("does not write a durable marker when capture sync deferral cannot start", async () => {
    beginCloudsyncActivityMock.mockRejectedValueOnce(
      new Error("cloudsync busy"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(sonnerToastErrorMock).toHaveBeenCalledWith(
      "Anarlog could not safely start recording. Please try again.",
      { id: "capture-state-persist-failed" },
    );
    consoleError.mockRestore();
    consoleWarn.mockRestore();
  });

  test("waits for capture sync deferral before writing the durable marker", async () => {
    let resolveDeferral: (() => void) | undefined;
    beginCloudsyncActivityMock.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveDeferral = resolve;
      }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));
    let starting: Promise<void>;

    act(() => {
      starting = result.current();
    });
    await waitFor(() =>
      expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce(),
    );
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(startMock).not.toHaveBeenCalled();

    await act(async () => {
      resolveDeferral?.();
      await starting;
    });

    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledOnce();
    expect(startMock).toHaveBeenCalledOnce();
  });

  test("releases capture sync deferral when native recording start throws", async () => {
    startMock.mockRejectedValueOnce(new Error("native start failed"));
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
    );
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledBefore(
      endCloudsyncActivityMock,
    );
    expect(setLeftSidebarExpandedMock).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  test("reads keywords from the same pre-start snapshot as the transcript memo", async () => {
    const calls: string[] = [];
    vi.mocked(getSessionKeywords).mockImplementation(async () => {
      calls.push("keywords");
      return ["launch"];
    });
    beginCloudsyncActivityMock.mockImplementation(async () => {
      calls.push("begin-cloudsync-deferral");
    });
    saveCaptureLifecycleMarkerMock.mockImplementation(async () => {
      calls.push("persist-marker");
    });
    startMock.mockImplementation(async () => {
      calls.push("start");
      return true;
    });

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(calls).toEqual([
      "keywords",
      "begin-cloudsync-deferral",
      "persist-marker",
      "start",
    ]);
    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      keywords: ["launch"],
    });
    expect(beginCloudsyncActivityMock).toHaveBeenCalledBefore(
      saveCaptureLifecycleMarkerMock,
    );
    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledBefore(startMock);
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

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav", {
      deferAudioFinalization: true,
      promotion: { scope: "whole_session" },
    });
    expect(setBatchTranscriptionPendingMock.mock.calls).toEqual([
      ["session-1", true],
      ["session-1", false],
    ]);
    expect(
      setBatchTranscriptionPendingMock.mock.invocationCallOrder[0],
    ).toBeLessThan(runBatchMock.mock.invocationCallOrder[0]!);
    expect(runBatchMock.mock.invocationCallOrder[0]!).toBeLessThan(
      clearCaptureLifecycleMarkerMock.mock.invocationCallOrder[0]!,
    );
    expect(
      clearCaptureLifecycleMarkerMock.mock.invocationCallOrder[0]!,
    ).toBeLessThan(endCloudsyncActivityMock.mock.invocationCallOrder[0]!);
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
    expect(
      markSessionAudioTranscriptionCompleteMock.mock.invocationCallOrder[0]!,
    ).toBeLessThan(
      clearCaptureLifecycleMarkerMock.mock.invocationCallOrder[0]!,
    );
    expect(
      clearCaptureLifecycleMarkerMock.mock.invocationCallOrder[0]!,
    ).toBeLessThan(
      deleteProcessedAudioForRetentionMock.mock.invocationCallOrder[0]!,
    );
  });

  test("refines complete multi-speaker Pro transcripts after stop", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
        model: "cloud",
        baseUrl: "https://api.test/stt",
        apiKey: "token",
      },
    });
    useSessionParticipantHumanIdsMock.mockReturnValue([
      "user-1",
      "lex",
      "george",
    ]);
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const callbacks = startMock.mock.calls[0]?.[1];
    callbacks?.handlePersist?.({
      new_words: [
        {
          id: "live-word",
          text: "hello",
          start_ms: 0,
          end_ms: 500,
          channel: 1,
        },
      ],
      replaced_ids: [],
      partials: [],
    });
    await act(async () => {
      await callbacks?.onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav", {
      deferAudioFinalization: true,
      promotion: {
        scope: "current_capture",
        audioOffsetMs: 0,
        replaceTranscriptId: "generated-id",
        startedAt: expect.any(Number),
      },
    });
    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
      "session-1",
    );
  });

  test("waits for the native capture lease to release before stop settles", async () => {
    let finishLeaseRelease: (() => void) | undefined;
    endCloudsyncActivityMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finishLeaseRelease = resolve;
        }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    const stopped = onStopped?.("session-1", {
      durationSeconds: 42,
      audioPath: "/tmp/session.wav",
      requestedLiveTranscription: false,
      liveTranscriptionActive: false,
      needsBatchRepair: false,
    });
    let settled = false;
    void stopped?.then(() => {
      settled = true;
    });

    await waitFor(() => {
      expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
    });
    expect(settled).toBe(false);

    finishLeaseRelease?.();
    await act(async () => {
      await stopped;
    });
    expect(settled).toBe(true);
  });

  test("drains meeting chat persistence before releasing the capture sync lease", async () => {
    let finishMeetingChatPersistence: (() => void) | undefined;
    stopMeetingChatCaptureMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finishMeetingChatPersistence = resolve;
        }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    const stopped = onStopped?.("session-1", {
      durationSeconds: 42,
      audioPath: null,
      requestedLiveTranscription: false,
      liveTranscriptionActive: false,
      needsBatchRepair: false,
    });

    await waitFor(() => {
      expect(stopMeetingChatCaptureMock).toHaveBeenCalledOnce();
    });
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    finishMeetingChatPersistence?.();
    await act(async () => {
      await stopped;
    });

    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
  });

  test("flushes canonical note persistence before releasing the capture sync lease", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);
    let finishEditorFlush: (() => void) | undefined;
    flushCanonicalSessionEditorChangesMock.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishEditorFlush = resolve;
      }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    const stopped = onStopped?.("session-1", {
      durationSeconds: 42,
      audioPath: null,
      requestedLiveTranscription: false,
      liveTranscriptionActive: false,
      needsBatchRepair: false,
    });

    await waitFor(() => {
      expect(flushCanonicalSessionEditorChangesMock).toHaveBeenCalledWith(
        "session-1",
      );
    });
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();

    finishEditorFlush?.();
    await act(async () => {
      await stopped;
    });
    expect(flushCanonicalSessionEditorChangesMock).toHaveBeenCalledBefore(
      queueAutoEnhanceIfSummaryEmptyMock,
    );
    expect(flushCanonicalSessionEditorChangesMock).toHaveBeenCalledBefore(
      clearCaptureLifecycleMarkerMock,
    );
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledBefore(
      endCloudsyncActivityMock,
    );
  });

  test("retains the marker and capture lease until a failed editor flush recovers", async () => {
    flushCanonicalSessionEditorChangesMock.mockRejectedValueOnce(
      new Error("database is locked"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const startResult = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await startResult.result.current();
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

    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    const marker = {
      version: 1 as const,
      phase: "finalizing" as const,
      sessionId: "session-1",
      transcriptId: "generated-id",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 0,
      preserveExistingTranscript: false,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    attachLiveSessionMock.mockResolvedValue("inactive");
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const recoveryResult = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(recoveryResult.result.current()).resolves.toBe("inactive");
    });

    expect(flushCanonicalSessionEditorChangesMock).toHaveBeenCalledTimes(2);
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
    );
    expect(beginCloudsyncActivityMock.mock.calls).toEqual([
      ["capture", "session-1:generated-id"],
      ["capture", "session-1:generated-id"],
    ]);
    expect(endCloudsyncActivityMock.mock.calls).toEqual([
      ["capture", "session-1:generated-id"],
    ]);
    consoleError.mockRestore();
  });

  test("finishes audio-retention writes before releasing the capture sync lease", async () => {
    let finishAudioRetention: (() => void) | undefined;
    deleteProcessedAudioForRetentionMock.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishAudioRetention = resolve;
      }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    const stopped = onStopped?.("session-1", {
      durationSeconds: 42,
      audioPath: "/tmp/session.wav",
      requestedLiveTranscription: true,
      liveTranscriptionActive: true,
      needsBatchRepair: false,
    });

    await waitFor(() =>
      expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledOnce(),
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    finishAudioRetention?.();
    await act(async () => {
      await stopped;
    });
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledBefore(
      endCloudsyncActivityMock,
    );
  });

  test("keeps a completed capture successful while lease release retries in the background", async () => {
    vi.useFakeTimers();
    endCloudsyncActivityMock
      .mockRejectedValueOnce(new Error("failure 1"))
      .mockRejectedValueOnce(new Error("failure 2"))
      .mockRejectedValueOnce(new Error("failure 3"))
      .mockRejectedValueOnce(new Error("failure 4"))
      .mockResolvedValueOnce(undefined);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    const stopped = onStopped?.("session-1", {
      durationSeconds: 42,
      audioPath: "/tmp/session.wav",
      requestedLiveTranscription: false,
      liveTranscriptionActive: false,
      needsBatchRepair: false,
    });
    await vi.advanceTimersByTimeAsync(400);
    await expect(stopped).resolves.toBeUndefined();

    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
    );
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledBefore(
      endCloudsyncActivityMock,
    );
    expect(requestCaptureRecoveryMock).not.toHaveBeenCalled();
    expect(endCloudsyncActivityMock).toHaveBeenCalledTimes(3);

    await vi.advanceTimersByTimeAsync(5_000);
    expect(endCloudsyncActivityMock).toHaveBeenCalledTimes(4);
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledOnce();

    await vi.advanceTimersByTimeAsync(5_000);
    expect(endCloudsyncActivityMock).toHaveBeenCalledTimes(5);
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  test("uses the transcript identity for each native capture lease", async () => {
    vi.useFakeTimers();
    idMock
      .mockReturnValueOnce("transcript-1")
      .mockReturnValueOnce("transcript-2");
    startMock.mockResolvedValue(false);
    endCloudsyncActivityMock
      .mockRejectedValueOnce(new Error("failure 1"))
      .mockRejectedValueOnce(new Error("failure 2"))
      .mockRejectedValueOnce(new Error("failure 3"))
      .mockResolvedValue(undefined);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    const firstStart = result.current();
    await vi.advanceTimersByTimeAsync(400);
    await firstStart;

    await result.current();

    expect(beginCloudsyncActivityMock.mock.calls).toEqual([
      ["capture", "session-1:transcript-1"],
      ["capture", "session-1:transcript-2"],
    ]);
    expect(endCloudsyncActivityMock).toHaveBeenLastCalledWith(
      "capture",
      "session-1:transcript-2",
    );

    await vi.advanceTimersByTimeAsync(5_000);
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-1",
    );
    consoleError.mockRestore();
  });

  test("preserves audio when durable capture cleanup cannot commit", async () => {
    clearCaptureLifecycleMarkerMock.mockRejectedValueOnce(
      new Error("database is locked"),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    await expect(
      onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      }),
    ).rejects.toThrow("database is locked");
    expect(markSessionAudioTranscriptionCompleteMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("finalizes repair audio only after the durable summary acknowledgement", async () => {
    let acknowledgeSummary: (() => void) | undefined;
    queueAutoEnhanceIfSummaryEmptyMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          acknowledgeSummary = resolve;
        }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    const stopped = onStopped?.("session-1", {
      durationSeconds: 42,
      audioPath: "/tmp/session.wav",
      requestedLiveTranscription: false,
      liveTranscriptionActive: false,
      needsBatchRepair: false,
    });

    await waitFor(() => {
      expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledOnce();
    });
    expect(saveCaptureLifecycleMarkerMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ summaryMode: "if_empty" }),
    );
    const summaryMarkerCallOrder =
      saveCaptureLifecycleMarkerMock.mock.invocationCallOrder;
    expect(
      summaryMarkerCallOrder[summaryMarkerCallOrder.length - 1]!,
    ).toBeLessThan(
      queueAutoEnhanceIfSummaryEmptyMock.mock.invocationCallOrder[0]!,
    );
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      expect.objectContaining({ phase: "finalizing" }),
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();

    acknowledgeSummary?.();
    await act(async () => await stopped);

    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(
      markSessionAudioTranscriptionCompleteMock.mock.invocationCallOrder[0]!,
    ).toBeLessThan(
      clearCaptureLifecycleMarkerMock.mock.invocationCallOrder[0]!,
    );
    expect(
      clearCaptureLifecycleMarkerMock.mock.invocationCallOrder[0]!,
    ).toBeLessThan(endCloudsyncActivityMock.mock.invocationCallOrder[0]!);
    expect(
      clearCaptureLifecycleMarkerMock.mock.invocationCallOrder[0]!,
    ).toBeLessThan(
      deleteProcessedAudioForRetentionMock.mock.invocationCallOrder[0]!,
    );
  });

  test("preserves existing transcripts when transcript state is still loading", async () => {
    useSessionHasTranscriptMock.mockReturnValue(null);
    audioSourceMetadataMock
      .mockResolvedValueOnce({
        status: "ok",
        data: {
          createdAt: null,
          modifiedAt: null,
          durationMs: 10_000,
        },
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: {
          createdAt: null,
          modifiedAt: null,
          durationMs: 60_000,
        },
      });
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

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav", {
      deferAudioFinalization: true,
      promotion: {
        scope: "current_capture",
        audioOffsetMs: 10_000,
        startedAt: expect.any(Number),
      },
    });
  });

  test("reattaches persistence and post-stop processing after a renderer reload", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);
    transcriptExistsMock.mockResolvedValue(true);
    loadCaptureLifecycleMarkerMock.mockResolvedValue({
      version: 1,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
      provider: "anarlog",
      model: "am-test",
    });
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await result.current();
    });

    expect(beginCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
    const callbacks = attachLiveSessionMock.mock.calls[0]?.[1];
    expect(callbacks?.handlePersist).toBeTypeOf("function");
    expect(callbacks?.onStopped).toBeTypeOf("function");

    callbacks?.handlePersist?.({
      new_words: [
        {
          id: "word-after-reload",
          text: "preserved",
          start_ms: 60_000,
          end_ms: 60_500,
          channel: 0,
        },
      ],
      replaced_ids: [],
      partials: [],
    });
    await act(async () => {
      await callbacks?.onStopped?.("session-1", {
        durationSeconds: 61,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });

    expect(applyLiveTranscriptDeltaToDatabaseMock).toHaveBeenCalledWith(
      "transcript-before-reload",
      expect.objectContaining({
        new_words: [expect.objectContaining({ id: "word-after-reload" })],
      }),
    );
    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav", {
      deferAudioFinalization: true,
      promotion: {
        scope: "current_capture",
        audioOffsetMs: 10_000,
        replaceTranscriptId: "transcript-before-reload",
        startedAt: 1_000,
      },
    });
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
  });

  test("requeues recovery when post-stop finalization fails after reattaching", async () => {
    transcriptExistsMock.mockRejectedValueOnce(new Error("database is locked"));
    const marker = {
      version: 1,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    } as const;
    loadCaptureLifecycleMarkerMock.mockResolvedValue(marker);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result, unmount } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("attached");
    });

    const onStopped = attachLiveSessionMock.mock.calls[0]?.[1]?.onStopped;
    unmount();
    await act(async () => {
      await expect(
        onStopped?.("session-1", {
          durationSeconds: 61,
          audioPath: "/tmp/session.wav",
          requestedLiveTranscription: true,
          liveTranscriptionActive: true,
          needsBatchRepair: false,
        }),
      ).rejects.toThrow("database is locked");
    });

    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    loadCaptureLifecycleMarkerMock
      .mockReset()
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    attachLiveSessionMock.mockResolvedValue("inactive");
    const retry = renderHook(() => useResumeListeningLifecycle("session-1"));

    await act(async () => {
      await expect(retry.result.current()).resolves.toBe("inactive");
    });

    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
    consoleError.mockRestore();
  });

  test("acquires sync deferral before reattach can deliver an immediate live delta", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);
    transcriptExistsMock.mockResolvedValue(true);
    loadCaptureLifecycleMarkerMock.mockResolvedValue({
      version: 1,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    });
    let resolveDeferral: (() => void) | undefined;
    beginCloudsyncActivityMock.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveDeferral = resolve;
      }),
    );
    attachLiveSessionMock.mockImplementationOnce(
      async (_sessionId, callbacks) => {
        callbacks?.handlePersist?.({
          new_words: [
            {
              id: "word-during-reattach",
              text: "preserved",
              start_ms: 60_000,
              end_ms: 60_500,
              channel: 0,
            },
          ],
          replaced_ids: [],
          partials: [],
        });
        return "attached";
      },
    );
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );
    let resuming: Promise<"attached" | "inactive" | "error">;

    act(() => {
      resuming = result.current();
    });
    await waitFor(() =>
      expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce(),
    );
    expect(attachLiveSessionMock).not.toHaveBeenCalled();
    expect(applyLiveTranscriptDeltaToDatabaseMock).not.toHaveBeenCalled();

    await act(async () => {
      resolveDeferral?.();
      await expect(resuming).resolves.toBe("attached");
    });
    await waitFor(() =>
      expect(applyLiveTranscriptDeltaToDatabaseMock).toHaveBeenCalledWith(
        "transcript-before-reload",
        expect.objectContaining({
          new_words: [expect.objectContaining({ id: "word-during-reattach" })],
        }),
      ),
    );

    expect(beginCloudsyncActivityMock).toHaveBeenCalledBefore(
      attachLiveSessionMock,
    );
  });

  test("reattached stop still schedules a summary while transcript state loads", async () => {
    useSessionHasTranscriptMock.mockReturnValue(null);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await result.current();
    });

    expect(beginCloudsyncActivityMock).toHaveBeenCalledBefore(
      attachLiveSessionMock,
    );
    expect(beginCloudsyncActivityMock).toHaveBeenCalledBefore(
      saveCaptureLifecycleMarkerMock,
    );
    const onStopped = attachLiveSessionMock.mock.calls[0]?.[1]?.onStopped;
    await act(async () => {
      await onStopped?.("session-1", {
        durationSeconds: 61,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });

    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
  });

  test("retries sync deferral for a reattached active capture", async () => {
    beginCloudsyncActivityMock.mockRejectedValueOnce(
      new Error("temporary cloudsync failure"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("error");
    });
    await act(async () => {
      await expect(result.current()).resolves.toBe("attached");
    });

    expect(beginCloudsyncActivityMock).toHaveBeenCalledTimes(2);
    expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  test("keeps an attached capture leased when its marker retry budget is exhausted", async () => {
    saveCaptureLifecycleMarkerMock.mockRejectedValue(
      new Error("marker write failed"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current({ abandonOnFailure: true })).resolves.toBe(
        "attached",
      );
    });

    expect(attachLiveSessionMock).toHaveBeenCalledOnce();
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  test("finalizes a durable capture when stop happens before listeners reattach", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce({
        version: 1,
        sessionId: "session-1",
        transcriptId: "transcript-before-reload",
        startedAt: 1_000,
        createdAt: "2026-07-24T00:00:00.000Z",
        audioOffsetMs: 10_000,
        preserveExistingTranscript: true,
        ownerUserId: "user-1",
        memo: "Existing memo",
      })
      .mockResolvedValueOnce({
        version: 1,
        sessionId: "session-1",
        transcriptId: "transcript-before-reload",
        startedAt: 1_000,
        createdAt: "2026-07-24T00:00:00.000Z",
        audioOffsetMs: 10_000,
        preserveExistingTranscript: true,
        ownerUserId: "user-1",
        memo: "Existing memo",
      })
      .mockResolvedValueOnce(null);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/existing-session.mp3", {
      deferAudioFinalization: true,
      promotion: {
        scope: "current_capture",
        audioOffsetMs: 10_000,
        startedAt: 1_000,
      },
    });
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
    expect(beginCaptureRecoveryFinalizationMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(finishCaptureRecoveryFinalizationMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(beginCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
  });

  test("retries a durable summary without re-transcribing completed live text", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
      summaryMode: "regenerate" as const,
    };
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(runBatchMock).not.toHaveBeenCalled();
    expect(resetEnhanceTasksMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "transcript-before-reload",
    );
    expect(beginCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
  });

  test("keeps synthetic recovery ownership without repeating failure toasts", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    runBatchMock
      .mockRejectedValueOnce(new Error("temporary repair failure"))
      .mockResolvedValueOnce(undefined);
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("error");
    });
    expect(sonnerToastErrorMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(runBatchMock).toHaveBeenCalledTimes(2);
    expect(beginCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(finishCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  test("preserves and reloads recovery after a transient marker read failure", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    loadCaptureLifecycleMarkerMock
      .mockRejectedValueOnce(new Error("database is locked"))
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current({ abandonOnFailure: true })).resolves.toBe(
        "error",
      );
    });
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(runBatchMock).toHaveBeenCalledOnce();
    expect(beginCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(finishCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  test("releases recovery ownership after the retry budget is exhausted", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    runBatchMock.mockRejectedValue(new Error("temporary repair failure"));
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    loadCaptureLifecycleMarkerMock.mockResolvedValue(marker);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current({ abandonOnFailure: true })).resolves.toBe(
        "error",
      );
    });

    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "transcript-before-reload",
    );
    expect(beginCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(finishCaptureRecoveryFinalizationMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  test("hands a failed capture to recovery without ending its native lease", async () => {
    runBatchMock
      .mockRejectedValueOnce(new Error("temporary repair failure"))
      .mockResolvedValueOnce(undefined);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const startResult = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await startResult.result.current();
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

    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "generated-id",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 0,
      preserveExistingTranscript: false,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    attachLiveSessionMock.mockResolvedValue("inactive");
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const recoveryResult = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(recoveryResult.result.current()).resolves.toBe("inactive");
    });

    expect(beginCloudsyncActivityMock.mock.calls).toEqual([
      ["capture", "session-1:generated-id"],
      ["capture", "session-1:generated-id"],
    ]);
    expect(endCloudsyncActivityMock.mock.calls).toEqual([
      ["capture", "session-1:generated-id"],
    ]);
    consoleError.mockRestore();
  });

  test("keeps the recovery lease when transcript existence lookup throws", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    transcriptExistsMock
      .mockRejectedValueOnce(new Error("database is locked"))
      .mockResolvedValueOnce(false);
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("error");
    });
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  test("keeps recovery ownership when the completion marker read fails", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockRejectedValueOnce(new Error("database is locked"))
      .mockResolvedValueOnce(null);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).rejects.toThrow("database is locked");
    });
    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(runBatchMock).toHaveBeenCalledOnce();
    expect(beginCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(finishCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledTimes(2);
    expect(endCloudsyncActivityMock).toHaveBeenCalledTimes(2);
  });

  test("retries a transient recovery audio path failure without clearing state", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    audioPathMock
      .mockResolvedValueOnce({
        status: "error",
        error: "database is locked",
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: "/tmp/existing-session.mp3",
      });
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("error");
    });
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(runBatchMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(runBatchMock).toHaveBeenCalledOnce();
    expect(beginCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(finishCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  test("releases sync after reattach confirms there is no active capture to recover", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    loadCaptureLifecycleMarkerMock.mockResolvedValue(null);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(beginCloudsyncActivityMock).toHaveBeenCalledBefore(
      attachLiveSessionMock,
    );
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
  });

  test("does not synthesize a stop when the native snapshot is unavailable", async () => {
    attachLiveSessionMock.mockResolvedValue("error");
    loadCaptureLifecycleMarkerMock.mockResolvedValue({
      version: 1,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    });
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("error");
    });

    expect(runBatchMock).not.toHaveBeenCalled();
    expect(beginCaptureRecoveryFinalizationMock).not.toHaveBeenCalled();
    expect(finishCaptureRecoveryFinalizationMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalledWith(
      "capture",
      "session-1:transcript-before-reload",
    );
  });

  test("keeps one recovery lease across a snapshot error until inactivity is proven", async () => {
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    attachLiveSessionMock
      .mockResolvedValueOnce("error")
      .mockResolvedValueOnce("inactive");
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("error");
    });
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
  });

  test("waits for native stop ownership before retrying recovery", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    beginCaptureRecoveryFinalizationMock
      .mockReturnValueOnce(false)
      .mockReturnValueOnce(true);
    const marker = {
      version: 1,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("error");
    });

    expect(runBatchMock).not.toHaveBeenCalled();
    expect(finishCaptureRecoveryFinalizationMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(beginCaptureRecoveryFinalizationMock).toHaveBeenCalledTimes(2);
    expect(runBatchMock).toHaveBeenCalledOnce();
    expect(finishCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
  });

  test("preserves recovery state when another worker owns finalization", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    beginCaptureRecoveryFinalizationMock.mockReturnValue(false);
    const marker = {
      version: 1 as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    loadCaptureLifecycleMarkerMock.mockResolvedValue(marker);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current({ abandonOnFailure: true })).resolves.toBe(
        "error",
      );
    });

    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(finishCaptureRecoveryFinalizationMock).not.toHaveBeenCalled();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).toHaveBeenCalledOnce();
  });

  test("reuses the same recovery attempt when stop arrives after a snapshot error", async () => {
    let callbacks:
      | {
          onStopped?: (
            sessionId: string,
            details: {
              durationSeconds: number;
              audioPath: string | null;
              requestedLiveTranscription: boolean;
              liveTranscriptionActive: boolean;
              needsBatchRepair: boolean;
            },
          ) => Promise<void> | void;
        }
      | undefined;
    attachLiveSessionMock
      .mockImplementationOnce(async (_sessionId, options) => {
        callbacks = options;
        return "error";
      })
      .mockResolvedValueOnce("inactive");
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce({
        version: 1,
        sessionId: "session-1",
        transcriptId: "transcript-before-reload",
        startedAt: 1_000,
        createdAt: "2026-07-24T00:00:00.000Z",
        audioOffsetMs: 10_000,
        preserveExistingTranscript: true,
        ownerUserId: "user-1",
        memo: "Existing memo",
      })
      .mockResolvedValue(null);
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("error");
    });
    await act(async () => {
      await callbacks?.onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });
    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      expect.objectContaining({
        transcriptId: "transcript-before-reload",
        summaryMode: "regenerate",
      }),
    );
    expect(runBatchMock).toHaveBeenCalledTimes(1);
    expect(finishCaptureRecoveryFinalizationMock).not.toHaveBeenCalled();
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

  test("repairs from finalized audio when live transcript persistence fails", async () => {
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
    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav", {
      deferAudioFinalization: true,
      promotion: { scope: "whole_session" },
    });
    expect(sonnerToastErrorMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
      "session-1",
    );
    consoleError.mockRestore();
  });

  test("persists live transcript deltas in arrival order", async () => {
    let resolveCreate: (() => void) | undefined;
    createLiveTranscriptMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveCreate = resolve;
        }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const callbacks = startMock.mock.calls[0]?.[1];
    callbacks?.handlePersist?.({
      new_words: [
        {
          id: "word-1",
          text: "first",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
        },
      ],
      replaced_ids: [],
      partials: [],
    });
    callbacks?.handlePersist?.({
      new_words: [
        {
          id: "word-2",
          text: "second",
          start_ms: 100,
          end_ms: 200,
          channel: 0,
        },
      ],
      replaced_ids: [],
      partials: [],
    });

    await waitFor(() => {
      expect(createLiveTranscriptMock).toHaveBeenCalledTimes(1);
    });
    expect(applyLiveTranscriptDeltaToDatabaseMock).not.toHaveBeenCalled();

    resolveCreate?.();
    await waitFor(() => {
      expect(applyLiveTranscriptDeltaToDatabaseMock).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      await callbacks?.onStopped?.("session-1", {
        durationSeconds: 1,
        audioPath: null,
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });
  });

  test("does not summarize an incomplete live transcript without repair audio", async () => {
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
          text: "partial",
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
        audioPath: null,
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });

    expect(sonnerToastErrorMock).toHaveBeenCalledWith(
      "Anarlog could not save part of the live transcript.",
      { id: "live-transcript-persist-failed", duration: Infinity },
    );
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ phase: "finalizing" }),
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    consoleError.mockRestore();
  });

  test("keeps repair audio when both live persistence and batch repair fail", async () => {
    createLiveTranscriptMock.mockRejectedValueOnce(new Error("write failed"));
    runBatchMock.mockRejectedValueOnce(new Error("batch failed"));
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
          text: "partial",
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

    expect(sonnerToastErrorMock).toHaveBeenCalledWith(
      "Anarlog could not finish saving the transcript. The recording was kept so you can try again.",
      { id: "post-capture-transcript-incomplete", duration: Infinity },
    );
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ phase: "finalizing" }),
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    consoleError.mockRestore();
  });

  test("retains audio and skips summary when the batch repair fails", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);
    runBatchMock.mockRejectedValueOnce(new Error("upload failed"));
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

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
        needsBatchRepair: true,
      });
    });

    expect(sonnerToastErrorMock).toHaveBeenCalledWith(
      "Post-meeting transcription failed. The recording was kept so you can try again.",
      { id: "post-capture-batch-failed", duration: Infinity },
    );
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ phase: "finalizing" }),
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    consoleError.mockRestore();
  });

  test("stops automatic recovery for a terminal batch repair failure", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);
    runBatchMock.mockRejectedValueOnce(
      new Error(
        "Bad Request: failed to process audio: corrupt or unsupported data",
      ),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

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
        needsBatchRepair: true,
      });
    });

    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
    );
    expect(requestCaptureRecoveryMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    consoleError.mockRestore();
  });

  test("keeps audio and skips summary when record-only transcription fails", async () => {
    runBatchMock.mockRejectedValueOnce(new Error("upload failed"));
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

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

    expect(sonnerToastErrorMock).toHaveBeenCalledWith(
      "Anarlog could not finish saving the transcript. The recording was kept so you can try again.",
      { id: "post-capture-transcript-incomplete", duration: Infinity },
    );
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ phase: "finalizing" }),
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    consoleError.mockRestore();
  });

  test("stays quiet when a record-only stop leaves nothing to summarize", async () => {
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
    await act(async () => {
      await onStopped?.("session-1", {
        durationSeconds: 1,
        audioPath: null,
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      });
    });

    expect(runBatchMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
    expect(requestMainAutoEnhanceMock).not.toHaveBeenCalled();
  });

  test("does not auto-enhance after the user cancels the batch repair", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);
    runBatchMock.mockRejectedValueOnce(new Error("Transcription stopped."));

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

    expect(queueAutoEnhanceMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
    expect(sonnerToastErrorMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ phase: "finalizing" }),
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
  });

  test("forwards auto-enhance to the main window when no enhancer service exists", async () => {
    getEnhancerServiceMock.mockReturnValue(null);
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

    expect(requestMainAutoEnhanceMock).toHaveBeenCalledWith(
      "session-1",
      "regenerate",
    );
    expect(queueAutoEnhanceMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
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
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
    expect(markSessionAudioTranscriptionCompleteMock).toHaveBeenCalledWith(
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
    await waitFor(() => {
      expect(createLiveTranscriptMock).toHaveBeenCalledTimes(1);
    });
    resolveTranscriptWrite?.();
    await act(async () => await stopped);

    expect(createLiveTranscriptMock).toHaveBeenCalledTimes(1);
    expect(resetEnhanceTasksMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
  });

  test("surfaces a summary scheduling failure after the service exhausts retries", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);
    queueAutoEnhanceIfSummaryEmptyMock.mockRejectedValueOnce(
      new Error("constraint failed"),
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

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

    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledOnce();
    expect(sonnerToastErrorMock).toHaveBeenCalledWith(
      "The transcript was saved, but Anarlog could not start the summary. Try generating it again.",
      { id: "post-capture-summary-failed", duration: Infinity },
    );
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      expect.objectContaining({ phase: "finalizing" }),
    );
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    consoleError.mockRestore();
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

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav", {
      deferAudioFinalization: true,
      promotion: {
        scope: "current_capture",
        audioOffsetMs: 60_000,
        startedAt: expect.any(Number),
      },
    });
    expect(resetEnhanceTasksMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
  });

  test("replaces only the current live transcript when resumed capture needs batch repair", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    const callbacks = startMock.mock.calls[0]?.[1];
    callbacks?.handlePersist?.({
      new_words: [
        {
          id: "word-current-live",
          text: "partial",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
        },
      ],
      replaced_ids: [],
      partials: [],
    });

    await waitFor(() => {
      expect(createLiveTranscriptMock).toHaveBeenCalledOnce();
    });

    await act(async () => {
      await callbacks?.onStopped?.("session-1", {
        durationSeconds: 42,
        audioPath: "/tmp/session.wav",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: true,
      });
    });

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav", {
      deferAudioFinalization: true,
      promotion: {
        scope: "current_capture",
        audioOffsetMs: 60_000,
        replaceTranscriptId: "generated-id",
        startedAt: expect.any(Number),
      },
    });
    expect(softDeleteTranscriptMock).not.toHaveBeenCalled();
  });

  test("forces batch transcription for batch-only local models with realtime stored", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
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
        provider: "anarlog",
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

  test("starts capture with the selected microphone", async () => {
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language"
        ? "en"
        : key === "microphone_device"
          ? "External Microphone"
          : key === "consent_auto_send_chat" || key === "capture_meeting_chat"
            ? false
            : [],
    );

    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      mic_device: "External Microphone",
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
        provider: "anarlog",
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
        provider: "anarlog",
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

  test("bounds completed meeting disclosure history", async () => {
    const sessionIds = Array.from(
      { length: MAX_SENT_MEETING_DISCLOSURE_SESSIONS + 1 },
      (_, index) => `bounded-disclosure-session-${index}`,
    );

    for (const sessionId of sessionIds) {
      startMeetingRecordingDisclosure(sessionId, () => true);
    }

    await waitFor(() => {
      expect(sendMeetingChatMessageMock).toHaveBeenCalledTimes(
        sessionIds.length,
      );
    });

    startMeetingRecordingDisclosure(
      sessionIds[sessionIds.length - 1]!,
      () => true,
    );
    await Promise.resolve();
    expect(sendMeetingChatMessageMock).toHaveBeenCalledTimes(sessionIds.length);

    startMeetingRecordingDisclosure(sessionIds[0]!, () => true);
    await waitFor(() => {
      expect(sendMeetingChatMessageMock).toHaveBeenCalledTimes(
        sessionIds.length + 1,
      );
    });
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
        { id: "com.anarlog.dev", name: "Anarlog Dev" },
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
        ["com.anarlog.dev", "com.tinyspeck.slackmacgap"],
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
      { id: "meeting-disclosure-send-failed", duration: Infinity },
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
      { id: "meeting-disclosure-send-failed", duration: Infinity },
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
