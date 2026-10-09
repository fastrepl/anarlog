import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  commands as transcriptionCommands,
  events as transcriptionEvents,
} from "@anlg/plugin-transcription";

import { saveIncompleteCapture } from "./capture-result";
import {
  CLOUDSYNC_CAPTURE_LEASE_ATTEMPTS,
  getPostCaptureAction,
  getPostCaptureRepairReasons,
  sendMeetingRecordingDisclosure,
  useResumeListeningLifecycle,
  useStartListening,
  useStartListeningState,
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
  getLiveStartErrorMock,
  stopMock,
  getSessionModeMock,
  setBatchTranscriptionPendingMock,
  runBatchMock,
  useListenerMock,
  isSessionDeletedMock,
  useSessionMock,
  useSessionHasTranscriptMock,
  useSessionParticipantHumanIdsMock,
  getSessionParticipantHumanIdsMock,
  createLiveTranscriptMock,
  appendRecoveredTranscriptWordsMock,
  createNativeTranscriptPersistenceMock,
  flushLiveTranscriptDeltasToDatabaseMock,
  transcriptExistsMock,
  softDeleteTranscriptMock,
  saveCaptureLifecycleMarkerMock,
  loadCaptureLifecycleMarkerMock,
  markCaptureAudioSavedMock,
  clearCaptureAudioSavedMock,
  getCaptureSnapshotMock,
  getStoppedCaptureMock,
  acknowledgeStoppedCaptureMock,
  clearCaptureLifecycleMarkerMock,
  requestCaptureRecoveryMock,
  waitForSessionSearchIndexMock,
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
  toastWarningMock,
  deleteTranscribedCaptureAudioMock,
  toastErrorMock,
  toastDismissMock,
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
  emptyCaptureMock,
} = vi.hoisted(() => ({
  emptyCaptureMock: vi.fn(),
  queueAutoEnhanceMock: vi.fn(),
  queueAutoEnhanceIfSummaryEmptyMock: vi.fn(),
  resetEnhanceTasksMock: vi.fn(),
  requestAutoEnhanceMock: vi.fn(),
  attachLiveSessionMock: vi.fn(),
  beginCaptureRecoveryFinalizationMock: vi.fn(),
  finishCaptureRecoveryFinalizationMock: vi.fn(),
  canStartLiveSessionMock: vi.fn(),
  startMock: vi.fn(),
  getLiveStartErrorMock: vi.fn((): string | null => null),
  stopMock: vi.fn(),
  getSessionModeMock: vi.fn(),
  setBatchTranscriptionPendingMock: vi.fn(),
  runBatchMock: vi.fn(),
  useListenerMock: vi.fn(),
  isSessionDeletedMock: vi.fn(),
  useSessionMock: vi.fn(),
  useSessionHasTranscriptMock: vi.fn(),
  useSessionParticipantHumanIdsMock: vi.fn(),
  getSessionParticipantHumanIdsMock: vi.fn(),
  createLiveTranscriptMock: vi.fn(),
  appendRecoveredTranscriptWordsMock: vi.fn(),
  createNativeTranscriptPersistenceMock: vi.fn(),
  flushLiveTranscriptDeltasToDatabaseMock: vi.fn(),
  transcriptExistsMock: vi.fn(),
  softDeleteTranscriptMock: vi.fn(),
  saveCaptureLifecycleMarkerMock: vi.fn(),
  loadCaptureLifecycleMarkerMock: vi.fn(),
  markCaptureAudioSavedMock: vi.fn(() => Promise.resolve()),
  clearCaptureAudioSavedMock: vi.fn(() => Promise.resolve()),
  getCaptureSnapshotMock: vi.fn(),
  getStoppedCaptureMock: vi.fn(),
  acknowledgeStoppedCaptureMock: vi.fn(),
  clearCaptureLifecycleMarkerMock: vi.fn(),
  requestCaptureRecoveryMock: vi.fn(),
  waitForSessionSearchIndexMock: vi.fn(),
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
  toastWarningMock: vi.fn(),
  deleteTranscribedCaptureAudioMock: vi.fn(),
  toastErrorMock: vi.fn(),
  toastDismissMock: vi.fn(),
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

vi.mock("~/auth", () => ({
  useAuth: () => ({ getSessionForRequest: vi.fn(async () => null) }),
}));
vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    acknowledgeStoppedCapture: acknowledgeStoppedCaptureMock,
    getStoppedCapture: getStoppedCaptureMock,
    getCaptureAudioGaps: vi.fn(async () => ({ status: "ok", data: null })),
    isSupportedLanguagesLive: isSupportedLanguagesLiveMock,
    listCaptureAudioChunks: vi.fn(async () => ({ status: "ok", data: [] })),
    acknowledgeCaptureAudioChunk: vi.fn(async () => ({
      status: "ok",
      data: null,
    })),
    deleteTranscribedCaptureAudio: deleteTranscribedCaptureAudioMock,
    updateCaptureCredentials: vi.fn(async () => ({ status: "ok", data: null })),
    getCaptureSnapshot: getCaptureSnapshotMock,
    listStoppedCaptures: vi.fn(async () => ({ status: "ok", data: [] })),
  },
  events: {
    captureLifecycleEvent: { listen: vi.fn(async () => () => {}) },
    captureStatusEvent: { listen: vi.fn(async () => () => {}) },
  },
}));
vi.mock("./capture-result", () => ({
  saveIncompleteCapture: vi.fn(async () => {}),
  clearIncompleteCapture: vi.fn(async () => {}),
}));

vi.mock("./native-transcript-persistence", () => ({
  createNativeTranscriptPersistence: createNativeTranscriptPersistenceMock,
}));

vi.mock("./contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "macos",
}));

vi.mock("@anlg/plugin-permissions", () => ({
  commands: {
    checkPermission: vi.fn(async () => ({ status: "ok", data: "authorized" })),
  },
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: {
    listMicUsingApplications: listMicUsingApplicationsMock,
    sendMeetingChatMessage: sendMeetingChatMessageMock,
  },
}));

vi.mock("./meeting-consent-store", () => ({
  persistDisclosureAttempt: vi.fn(async () => {}),
  persistParticipantConsent: vi.fn(async () => {}),
}));

vi.mock("./empty-automatic-capture", () => ({
  discardEmptyAutomaticCapture: emptyCaptureMock,
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    audioExist: vi.fn().mockResolvedValue({ status: "ok", data: false }),
    audioPath: audioPathMock,
    audioSourceMetadata: audioSourceMetadataMock,
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: {
    warning: toastWarningMock,
    error: toastErrorMock,
    dismiss: toastDismissMock,
    info: vi.fn(),
  },
}));

vi.mock("~/ai/task-window-sync", () => ({
  requestMainAutoEnhance: requestMainAutoEnhanceMock,
}));

vi.mock("./primary-device", () => ({
  consumePrimaryDeviceYield: () => false,
  startPrimaryDeviceCoordination: vi.fn(),
}));

vi.mock("./meeting-chat-capture", () => ({
  startMeetingChatCapture: startMeetingChatCaptureMock,
}));

vi.mock("./meeting-screen-capture", () => ({
  startMeetingScreenCapture: vi.fn(() => async () => {}),
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
  reconcileRefinedSpeakerClusters: vi.fn(
    async (_source: unknown, _words: unknown, hints: unknown) => hints,
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
  isSessionDeleted: isSessionDeletedMock,
  useSession: useSessionMock,
  useSessionTranscriptExistence: useSessionHasTranscriptMock,
}));

vi.mock("~/session-sharing/editor-activity", () => ({
  flushCanonicalSessionEditorChanges: flushCanonicalSessionEditorChangesMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/shared/completion-sound", () => ({
  playCompletionSound: vi.fn().mockResolvedValue(undefined),
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
  hasAudioAwaitingUser: (marker: {
    chunkedAudio?: boolean;
    summaryMode?: string;
    inheritedCaptures?: unknown[];
  }) =>
    !marker.summaryMode &&
    (marker.chunkedAudio === true ||
      (marker.inheritedCaptures ?? []).length > 0),
  hasPendingZeroRetentionAudio: (marker: {
    chunkedAudio?: boolean;
    retainAudio?: boolean;
  }) => marker.chunkedAudio === true && marker.retainAudio === false,
  loadCaptureLifecycleMarker: loadCaptureLifecycleMarkerMock,
  saveCaptureLifecycleMarker: saveCaptureLifecycleMarkerMock,
  markCaptureAudioSaved: markCaptureAudioSavedMock,
  clearCaptureAudioSaved: clearCaptureAudioSavedMock,
}));

vi.mock("~/stt/capture-recovery-requests", () => ({
  requestCaptureRecovery: requestCaptureRecoveryMock,
}));

vi.mock("~/stt/search-index-consistency", () => ({
  waitForSessionSearchIndex: waitForSessionSearchIndexMock,
}));

vi.mock("~/stt/queries", () => ({
  appendRecoveredTranscriptWords: appendRecoveredTranscriptWordsMock,
  createLiveTranscript: createLiveTranscriptMock,
  flushLiveTranscriptDeltasToDatabase: flushLiveTranscriptDeltasToDatabaseMock,
  getTranscriptRecord: vi.fn(async () => null),
  softDeleteTranscript: softDeleteTranscriptMock,
  transcriptExists: transcriptExistsMock,
  useSessionParticipantHumanIds: useSessionParticipantHumanIdsMock,
  getSessionParticipantHumanIds: getSessionParticipantHumanIdsMock,
}));

let disclosureSessionSequence = 0;

function nextDisclosureSessionId() {
  disclosureSessionSequence += 1;
  return `disclosure-session-${disclosureSessionSequence}`;
}

function failNextNativePersistenceFlush() {
  createNativeTranscriptPersistenceMock.mockImplementationOnce(
    ({ onError }) => ({
      flush: vi.fn(async () => onError(new Error("write failed"))),
      hasPendingFailure: vi.fn(() => true),
      dispose: vi.fn(),
    }),
  );
}

function succeedNextNativePersistenceFlush() {
  createNativeTranscriptPersistenceMock.mockImplementationOnce(
    ({ afterFlush, onPersisted, sessionId, transcriptId }) => ({
      flush: vi.fn(async () => {
        onPersisted({
          session_id: sessionId,
          transcript_id: transcriptId,
          transcript_created: true,
          persisted_through_ms: 500,
          error: null,
        });
        await afterFlush();
      }),
      hasPendingFailure: vi.fn(() => false),
      dispose: vi.fn(),
    }),
  );
}

describe("getPostCaptureAction", () => {
  test.each([
    {
      name: "reports every reason that requires post-stop transcript repair",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: false,
        needsBatchRepair: true,
        transcriptWriteFailed: true,
      },
      expected: [
        "live_transcription_unavailable",
        "live_stream_incomplete",
        "transcript_persistence_failed",
      ],
    },
    {
      name: "reports no repair reason for a complete persisted live transcript",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      },
      expected: [],
    },
    {
      name: "reports settled diarization refinement for a multi-speaker cloud transcript",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
        refineSpeakerDiarization: true,
      },
      expected: ["settled_speaker_diarization"],
    },
  ])("$name", ({ input, expected }) => {
    expect(getPostCaptureRepairReasons(input)).toEqual(expected);
  });

  test.each([
    {
      name: "runs batch then enhance after record-only capture finishes when audio is available",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      },
      canBatch: true,
      expected: "batch_then_enhance",
    },
    {
      name: "enhances immediately when live transcription already completed during recording",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      },
      canBatch: true,
      expected: "enhance_only",
    },
    {
      name: "refines a complete live transcript when settled diarization is required",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
        refineSpeakerDiarization: true,
      },
      canBatch: true,
      expected: "batch_then_enhance",
    },
    {
      name: "keeps a complete live transcript when settled diarization cannot run",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
        refineSpeakerDiarization: true,
      },
      canBatch: false,
      expected: "enhance_only",
    },
    {
      name: "repairs the full transcript after live transcription recovered",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: true,
        needsBatchRepair: true,
      },
      canBatch: true,
      expected: "batch_then_enhance",
    },
    {
      name: "repairs the full transcript after a live database write fails",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
        transcriptWriteFailed: true,
      },
      canBatch: true,
      expected: "batch_then_enhance",
    },
    {
      name: "does nothing when batch fallback is needed but no transcription connection is available",
      input: {
        audioPath: "/tmp/session.wav" as string | null,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      },
      canBatch: false,
      expected: "none",
    },
    {
      name: "does nothing when capture finishes without a saved audio path",
      input: {
        audioPath: null as string | null,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      },
      canBatch: true,
      expected: "none",
    },
  ])("$name", ({ input, canBatch, expected }) => {
    expect(getPostCaptureAction(input, canBatch)).toBe(expected);
  });
});

describe("useStartListening", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getCaptureSnapshotMock.mockResolvedValue({
      status: "ok",
      data: { activeSessionId: null, finalizingSessionIds: [] },
    });
    getStoppedCaptureMock.mockResolvedValue({ status: "ok", data: null });
    acknowledgeStoppedCaptureMock.mockResolvedValue({
      status: "ok",
      data: null,
    });
    emptyCaptureMock.mockResolvedValue(false);
    idMock.mockReturnValue("generated-id");

    getEnhancerServiceMock.mockImplementation(() => ({
      queueAutoEnhance: queueAutoEnhanceMock,
      queueAutoEnhanceIfSummaryEmpty: queueAutoEnhanceIfSummaryEmptyMock,
      resetEnhanceTasks: resetEnhanceTasksMock,
      requestAutoEnhance: requestAutoEnhanceMock,
    }));
    requestAutoEnhanceMock.mockImplementation(
      async (
        targetSessionId: string,
        mode: "regenerate" | "if_empty" | "refresh",
      ) => {
        if (mode !== "regenerate") {
          const result =
            await queueAutoEnhanceIfSummaryEmptyMock(targetSessionId);
          if (mode === "if_empty" || result?.type !== "summary_exists") return;
        }
        await resetEnhanceTasksMock(targetSessionId);
        queueAutoEnhanceMock(targetSessionId);
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
        getLiveStartError: getLiveStartErrorMock,
        setBatchTranscriptionPending: setBatchTranscriptionPendingMock,
        start: startMock,
        stop: stopMock,
      }),
    );
    beginCaptureRecoveryFinalizationMock.mockReturnValue(true);
    canStartLiveSessionMock.mockReturnValue(true);
    getSessionModeMock.mockReturnValue("active");
    isSessionDeletedMock.mockResolvedValue(false);
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "Existing memo",
    });
    useSessionHasTranscriptMock.mockReturnValue(false);
    useSessionParticipantHumanIdsMock.mockReturnValue([]);
    getSessionParticipantHumanIdsMock.mockImplementation(async () =>
      useSessionParticipantHumanIdsMock(),
    );
    createLiveTranscriptMock.mockResolvedValue(undefined);
    createNativeTranscriptPersistenceMock.mockImplementation(
      ({ afterFlush }) => ({
        flush: vi.fn(async () => afterFlush()),
        hasPendingFailure: vi.fn(() => false),
        dispose: vi.fn(),
      }),
    );
    flushLiveTranscriptDeltasToDatabaseMock.mockResolvedValue(undefined);
    transcriptExistsMock.mockResolvedValue(false);
    softDeleteTranscriptMock.mockResolvedValue(undefined);
    saveCaptureLifecycleMarkerMock.mockResolvedValue(undefined);
    loadCaptureLifecycleMarkerMock.mockResolvedValue(null);
    clearCaptureLifecycleMarkerMock.mockResolvedValue(undefined);
    requestCaptureRecoveryMock.mockResolvedValue(undefined);
    waitForSessionSearchIndexMock.mockResolvedValue(undefined);
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
    vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue({
      status: "ok",
      data: [],
    });
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
    deleteTranscribedCaptureAudioMock.mockResolvedValue({
      status: "ok",
      data: true,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("zero retention keeps untranscribed audio at stop after a disconnect", async () => {
    deleteTranscribedCaptureAudioMock.mockResolvedValue({
      status: "ok",
      data: false,
    });
    useConfigValueMock.mockImplementation((key: string) =>
      key === "audio_retention" ? "none" : undefined,
    );
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    expect(startMock.mock.calls[0]?.[0]).toMatchObject({ retain_audio: false });
    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      live_transcript: expect.objectContaining({
        transcript_id: "generated-id",
        owner_user_id: "user-1",
        created_at: expect.any(String),
        started_at_ms: expect.any(Number),
        memo: "Existing memo",
      }),
    });
    const progress = vi.mocked(transcriptionEvents.captureStatusEvent.listen)
      .mock.calls[0]?.[0];
    progress?.({
      payload: {
        type: "connection_error",
        session_id: "session-1",
        error: "offline",
      },
    } as never);
    await act(async () => {
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        durationSeconds: 60,
        audioPath: "/tmp/session.mp3",
        requestedLiveTranscription: true,
        liveTranscriptionActive: false,
        needsBatchRepair: true,
      });
    });
    expect(runBatchMock).not.toHaveBeenCalled();
    expect(deleteTranscribedCaptureAudioMock).toHaveBeenCalledWith("session-1");
    expect(saveIncompleteCapture).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
      false,
      false,
      true,
    );
    expect(toastWarningMock).toHaveBeenCalledWith(
      "Audio kept to finish your transcript",
      expect.anything(),
    );
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("zero retention deletes temporary audio once transcription succeeds", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "audio_retention" ? "none" : undefined,
    );
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    await act(async () => {
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        durationSeconds: 60,
        audioPath: "/tmp/session.mp3",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });
    expect(deleteTranscribedCaptureAudioMock).toHaveBeenCalledWith("session-1");
    expect(saveIncompleteCapture).not.toHaveBeenCalled();
    expect(toastWarningMock).not.toHaveBeenCalledWith(
      "Audio kept to finish your transcript",
      expect.anything(),
    );
  });

  test("does not reprocess a whole chunked recording after recovery has completed", async () => {
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    await act(async () => {
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        durationSeconds: 60,
        audioPath: "/tmp/session.mp3",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: true,
      });
    });
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("transcribes retained Scribe V2 audio only after chunked capture stops", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "elevenlabs",
        model: "scribe_v2",
        baseUrl: "https://api.elevenlabs.io/v1",
        apiKey: "token",
      },
    });
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    expect(startMock.mock.calls[0]?.[0]).toMatchObject({
      transcription_mode: "batch",
      retain_audio: true,
    });
    expect(runBatchMock).not.toHaveBeenCalled();

    await act(async () => {
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        durationSeconds: 60,
        audioPath: "/tmp/session.mp3",
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      });
    });

    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.mp3", {
      deferAudioFinalization: true,
      notifyOnCompletion: true,
      promotion: { scope: "whole_session" },
    });
  });

  test("a new recording adopts untranscribed zero-retention audio from the same note", async () => {
    const pending = {
      version: 1,
      chunkedAudio: true,
      retainAudio: false,
      sessionId: "session-1",
      transcriptId: "crashed-transcript",
      startedAt: 1_000,
      createdAt: "2026-01-01T00:00:00.000Z",
      audioOffsetMs: 0,
      preserveExistingTranscript: false,
      ownerUserId: "user-1",
      memo: "",
    };
    loadCaptureLifecycleMarkerMock.mockResolvedValueOnce(pending);
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      expect.objectContaining({
        transcriptId: "generated-id",
        inheritedCaptures: [
          expect.objectContaining({
            transcriptId: "crashed-transcript",
            startedAt: 1_000,
            retainAudio: false,
          }),
        ],
      }),
      "crashed-transcript",
    );
  });

  test("hands earlier untranscribed audio to recovery after a retained batch completes", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "elevenlabs",
        model: "scribe_v2",
        baseUrl: "https://api.elevenlabs.io/v1",
        apiKey: "token",
      },
    });
    loadCaptureLifecycleMarkerMock.mockResolvedValueOnce({
      version: 1,
      chunkedAudio: true,
      retainAudio: false,
      sessionId: "session-1",
      transcriptId: "crashed-transcript",
      startedAt: 1_000,
      createdAt: "2026-01-01T00:00:00.000Z",
      audioOffsetMs: 0,
      preserveExistingTranscript: false,
      ownerUserId: "user-1",
      memo: "",
    });
    vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue({
      status: "ok",
      data: [
        {
          id: "1000-0-60000-0.mp3",
          path: "/earlier.mp3",
          capture_started_at: 1_000,
          start_ms: 0,
          audio_start_ms: 0,
          end_ms: 60_000,
        },
      ],
    });
    runBatchMock.mockRejectedValueOnce(new Error("offline"));
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        durationSeconds: 60,
        audioPath: "/tmp/session.mp3",
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      });
    });

    expect(runBatchMock).toHaveBeenCalledWith(
      "/tmp/session.mp3",
      expect.objectContaining({ promotion: { scope: "whole_session" } }),
    );
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).toHaveBeenLastCalledWith(
      expect.objectContaining({
        transcriptId: "generated-id",
        phase: "finalizing",
        inheritedOnly: true,
        inheritedCaptures: [
          expect.objectContaining({ transcriptId: "crashed-transcript" }),
        ],
      }),
    );
    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue({
      status: "ok",
      data: [],
    });
    consoleError.mockRestore();
    consoleWarn.mockRestore();
  });

  test("transcribes a retained batch-only recording once after stop instead of per chunk", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const capturedAt = new Date("2026-07-24T00:00:00.000Z").getTime();
    vi.setSystemTime(capturedAt);
    vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue({
      status: "ok",
      data: [
        {
          id: "chunk-1",
          path: "/tmp/chunk-1.ogg",
          capture_started_at: capturedAt,
          start_ms: 0,
          audio_start_ms: 0,
          end_ms: 60_000,
        },
      ],
    });
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    vi.setSystemTime(capturedAt + 120_000);
    const lifecycle = vi.mocked(
      transcriptionEvents.captureLifecycleEvent.listen,
    ).mock.calls[0]?.[0];
    lifecycle?.({
      payload: {
        type: "started",
        session_id: "session-1",
        requested_live_transcription: false,
        live_transcription_active: false,
      },
    } as never);
    await act(async () => {
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        durationSeconds: 120,
        audioPath: "/tmp/session.mp3",
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      });
    });
    expect(runBatchMock).toHaveBeenCalledOnce();
    expect(runBatchMock).toHaveBeenCalledWith(
      "/tmp/session.mp3",
      expect.objectContaining({ notifyOnCompletion: true }),
    );
    expect(setBatchTranscriptionPendingMock).toHaveBeenCalledWith(
      "session-1",
      true,
    );
  });

  test("keeps zero-retention batch-only recordings on the in-meeting chunk path", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "audio_retention" ? "none" : undefined,
    );
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    const lifecycle = vi.mocked(
      transcriptionEvents.captureLifecycleEvent.listen,
    ).mock.calls[0]?.[0];
    lifecycle?.({
      payload: {
        type: "started",
        session_id: "session-1",
        requested_live_transcription: false,
        live_transcription_active: false,
      },
    } as never);
    await act(async () => {
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        durationSeconds: 60,
        audioPath: "/tmp/session.mp3",
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      });
    });
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("repairs retained batch-only chunks when the full recording is missing at stop", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const capturedAt = new Date("2026-07-24T00:00:00.000Z").getTime();
    vi.setSystemTime(capturedAt);
    vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue({
      status: "ok",
      data: [
        {
          id: "chunk-1",
          path: "/tmp/chunk-1.ogg",
          capture_started_at: capturedAt,
          start_ms: 0,
          audio_start_ms: 0,
          end_ms: 60_000,
        },
      ],
    });
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    vi.setSystemTime(capturedAt + 120_000);
    const lifecycle = vi.mocked(
      transcriptionEvents.captureLifecycleEvent.listen,
    ).mock.calls[0]?.[0];
    lifecycle?.({
      payload: {
        type: "started",
        session_id: "session-1",
        requested_live_transcription: false,
        live_transcription_active: false,
      },
    } as never);
    await act(async () => {
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        durationSeconds: 120,
        audioPath: null,
        requestedLiveTranscription: false,
        liveTranscriptionActive: false,
        needsBatchRepair: false,
      });
    });
    expect(runBatchMock.mock.calls.map(([path]) => path)).toEqual([
      "/tmp/chunk-1.ogg",
    ]);
  });

  describe("retained batch-only recovery", () => {
    const batchOnlyMarker = {
      version: 1 as const,
      chunkedAudio: true,
      retainAudio: true,
      postStopBatch: true,
      phase: "finalizing" as const,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: Date.now() - 180_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 0,
      preserveExistingTranscript: false,
      ownerUserId: "user-1",
      memo: "Existing memo",
    };
    const leftoverChunk = {
      id: "chunk-1",
      path: "/tmp/chunk-1.ogg",
      capture_started_at: Date.now() - 180_000,
      start_ms: 0,
      audio_start_ms: 0,
      end_ms: 60_000,
    };
    const recoverStoppedCapture = async () => {
      attachLiveSessionMock.mockResolvedValue("inactive");
      loadCaptureLifecycleMarkerMock
        .mockResolvedValueOnce(batchOnlyMarker)
        .mockResolvedValueOnce(batchOnlyMarker)
        .mockResolvedValueOnce(null);
      const { result } = renderHook(() =>
        useResumeListeningLifecycle("session-1"),
      );
      await act(async () => {
        await result.current({ processStopped: true });
      });
    };

    test("reruns the full-file batch after a failed pass left no chunks", async () => {
      await recoverStoppedCapture();

      expect(runBatchMock).toHaveBeenCalledOnce();
      expect(runBatchMock).toHaveBeenCalledWith(
        "/tmp/existing-session.mp3",
        expect.objectContaining({ deferAudioFinalization: true }),
      );
    });

    test("transcribes the full file instead of leftover chunks after a restart", async () => {
      vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue(
        { status: "ok", data: [leftoverChunk] },
      );

      await recoverStoppedCapture();

      expect(runBatchMock.mock.calls.map(([path]) => path)).toEqual([
        "/tmp/existing-session.mp3",
      ]);
    });

    test("repairs leftover chunks after a restart when the full recording is missing", async () => {
      vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue(
        { status: "ok", data: [leftoverChunk] },
      );
      audioPathMock.mockResolvedValue({
        status: "error",
        error: "audio_path_not_found",
      });

      await recoverStoppedCapture();

      expect(runBatchMock.mock.calls.map(([path]) => path)).toEqual([
        "/tmp/chunk-1.ogg",
      ]);
    });

    test("does not transcribe chunks after reattaching to a running capture", async () => {
      vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue(
        { status: "ok", data: [leftoverChunk] },
      );
      loadCaptureLifecycleMarkerMock.mockResolvedValue({
        ...batchOnlyMarker,
        phase: "capturing",
      });
      const { result } = renderHook(() =>
        useResumeListeningLifecycle("session-1"),
      );
      await act(async () => {
        await expect(result.current()).resolves.toBe("attached");
      });

      await act(async () => {
        await attachLiveSessionMock.mock.calls[0]?.[1]?.onStopped?.(
          "session-1",
          {
            chunkedAudio: true,
            durationSeconds: 180,
            audioPath: "/tmp/session.mp3",
            requestedLiveTranscription: false,
            liveTranscriptionActive: false,
            needsBatchRepair: false,
          },
        );
      });

      expect(runBatchMock.mock.calls.map(([path]) => path)).toEqual([
        "/tmp/session.mp3",
      ]);
    });
  });

  test.each([
    ["batch-only with retained audio", "am-test", undefined, true],
    ["live", "soniqo-parakeet-streaming", undefined, false],
    ["batch-only with zero retention", "am-test", "none", false],
  ])(
    "persists the post-stop batch mode on the first marker: %s",
    async (_label, model, retention, expected) => {
      useSTTConnectionMock.mockReturnValue({
        conn: {
          provider: "anarlog",
          model,
          baseUrl: "http://localhost:8080",
          apiKey: "",
        },
      });
      if (retention)
        useConfigValueMock.mockImplementation((key: string) =>
          key === "audio_retention" ? retention : undefined,
        );
      const { result } = renderHook(() => useStartListening("session-1"));
      await act(async () => {
        await result.current();
      });

      const firstMarker = saveCaptureLifecycleMarkerMock.mock.calls[0]?.[0];
      expect(firstMarker?.postStopBatch === true).toBe(expected);
    },
  );

  test("never claims that zero-retention audio was deleted when native cleanup failed", async () => {
    useConfigValueMock.mockImplementation((key: string) =>
      key === "audio_retention" ? "none" : undefined,
    );
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    await act(async () => {
      await startMock.mock.calls[0]?.[1].onStopped("session-1", {
        chunkedAudio: true,
        audioDeletionFailed: true,
        durationSeconds: 60,
        audioPath: "/tmp/session.mp3",
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      });
    });
    expect(saveIncompleteCapture).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
      false,
      true,
      false,
    );
    expect(toastErrorMock).toHaveBeenCalledWith(
      "Audio could not be deleted",
      expect.anything(),
    );
    expect(toastErrorMock).not.toHaveBeenCalledWith(
      "Your transcript is incomplete",
      expect.anything(),
    );
    expect(runBatchMock).not.toHaveBeenCalled();
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
    expect(toastWarningMock).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        id: "recording-without-transcription",
        action: expect.anything(),
      }),
    );

    const warningCalls = toastWarningMock.mock.calls;
    const warningOptions = warningCalls[warningCalls.length - 1]?.[1];
    warningOptions?.action.onClick();

    expect(openNewMock).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "transcription" },
    });
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

  test("keeps recording when capture sync deferral cannot be acquired", async () => {
    beginCloudsyncActivityMock.mockRejectedValue(new Error("cloudsync busy"));
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });
    await waitFor(() =>
      expect(beginCloudsyncActivityMock).toHaveBeenCalledTimes(
        CLOUDSYNC_CAPTURE_LEASE_ATTEMPTS,
      ),
    );

    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledOnce();
    expect(startMock).toHaveBeenCalledOnce();
    expect(toastErrorMock).not.toHaveBeenCalled();
    consoleError.mockRestore();
    consoleWarn.mockRestore();
  });

  test("retries capture sync deferral after a transient drain timeout", async () => {
    beginCloudsyncActivityMock.mockRejectedValueOnce(
      new Error("CloudSync activity could not drain the in-flight operation"),
    );
    const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });
    await waitFor(() =>
      expect(beginCloudsyncActivityMock).toHaveBeenCalledTimes(2),
    );

    expect(startMock).toHaveBeenCalledOnce();
    expect(toastErrorMock).not.toHaveBeenCalled();
    consoleWarn.mockRestore();
  });

  test("starts capture without waiting for the sync deferral to settle", async () => {
    let resolveDeferral: (() => void) | undefined;
    beginCloudsyncActivityMock.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveDeferral = resolve;
      }),
    );
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledOnce();
    expect(startMock).toHaveBeenCalledOnce();

    await act(async () => {
      resolveDeferral?.();
    });
  });

  test("stops retrying the sync deferral once the capture has ended", async () => {
    let rejectDeferral: ((error: Error) => void) | undefined;
    beginCloudsyncActivityMock.mockReturnValueOnce(
      new Promise<void>((_, reject) => {
        rejectDeferral = reject;
      }),
    );
    const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });
    expect(startMock).toHaveBeenCalledOnce();

    const [, options] = startMock.mock.calls[0];
    const stopped = options.onStopped("session-1", {
      durationSeconds: 1,
      audioPath: null,
      requestedLiveTranscription: true,
      liveTranscriptionActive: true,
      needsBatchRepair: false,
    });
    // Finalization reaches the lease release right after the retention step;
    // the pending acquisition must not be retried once release was requested.
    await waitFor(() =>
      expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalled(),
    );
    await act(async () => {
      rejectDeferral?.(new Error("activity ended before idle"));
      await stopped;
    });

    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    consoleWarn.mockRestore();
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

  test("manual recording stays manual while a scheduled start is waiting for the same note", async () => {
    renderHook(() => useStartListeningState("session-1", { automatic: true }));
    const { result } = renderHook(() => useStartListening("session-1"));
    await act(async () => {
      await result.current();
    });
    expect(saveCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      expect.objectContaining({
        automatic: false,
        preserveExistingAudio: true,
      }),
    );
  });

  test.each([true, false])(
    "discards an empty scheduled capture before publishing audio or running batch transcription (live=%s)",
    async (live) => {
      emptyCaptureMock.mockResolvedValue(true);
      useSessionMock.mockReturnValue({
        id: "session-1",
        user_id: "user-1",
        raw_md: "",
        title: "Standup",
      });
      const { result } = renderHook(
        () =>
          useStartListeningState("session-1", { automatic: true })
            .startListening,
      );
      await act(async () => {
        await result.current();
      });
      const onStopped = startMock.mock.calls[0]?.[1]?.onStopped;
      await act(async () => {
        await onStopped("session-1", {
          durationSeconds: 42,
          audioPath: "/tmp/session.wav",
          requestedLiveTranscription: live,
          liveTranscriptionActive: live,
          needsBatchRepair: false,
        });
      });
      expect(emptyCaptureMock).toHaveBeenCalledWith(
        expect.objectContaining({
          automatic: true,
          preserveExistingAudio: false,
          initialTitle: "Standup",
          transcriptionComplete: true,
        }),
      );
      expect(catalogLocalSessionAudioMock).not.toHaveBeenCalled();
      expect(runBatchMock).not.toHaveBeenCalled();
      expect(requestMainAutoEnhanceMock).not.toHaveBeenCalled();
      expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
        "session-1",
        "generated-id",
      );
      expect(endCloudsyncActivityMock).toHaveBeenCalled();
    },
  );

  test("does not treat a capture with a committed native transcript as empty", async () => {
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "",
      title: "Standup",
    });
    const { result } = renderHook(
      () =>
        useStartListeningState("session-1", { automatic: true }).startListening,
    );
    await act(async () => {
      await result.current();
    });

    const onPersisted =
      createNativeTranscriptPersistenceMock.mock.calls[0]?.[0]?.onPersisted;
    act(() => {
      onPersisted?.({
        session_id: "session-1",
        transcript_id: "generated-id",
        transcript_created: true,
        persisted_through_ms: 500,
        error: null,
      });
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

    expect(emptyCaptureMock).toHaveBeenCalledWith(
      expect.objectContaining({ transcriptTouched: true }),
    );
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
      notifyOnCompletion: true,
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
    succeedNextNativePersistenceFlush();
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
      notifyOnCompletion: false,
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

  test.each([
    { provider: "soniqo", model: "soniqo-parakeet-streaming" },
    { provider: "apple_speech", model: "apple-speech" },
  ])(
    "refines complete multi-speaker $model transcripts with the installed local batch model",
    async ({ provider, model }) => {
      succeedNextNativePersistenceFlush();
      useSTTConnectionMock.mockReturnValue({
        conn: {
          provider,
          model,
          baseUrl: "soniqo://local",
          apiKey: "",
        },
        localBatchDiarizationAvailable: true,
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
        notifyOnCompletion: false,
        provider: "soniqo",
        model: "soniqo-parakeet-batch",
        baseUrl: "soniqo://local",
        apiKey: "",
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
    },
  );

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
      await expect(
        recoveryResult.result.current({ processStopped: true }),
      ).resolves.toBe("inactive");
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
      expect.objectContaining({ summaryMode: "refresh" }),
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
      notifyOnCompletion: true,
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
      await result.current({ processStopped: true });
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

    expect(flushLiveTranscriptDeltasToDatabaseMock).toHaveBeenCalledWith(
      "transcript-before-reload",
    );
    expect(runBatchMock).toHaveBeenCalledWith("/tmp/session.wav", {
      deferAudioFinalization: true,
      notifyOnCompletion: false,
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "attached",
      );
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
      await expect(
        retry.result.current({ processStopped: true }),
      ).resolves.toBe("inactive");
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
    let resuming: Promise<"attached" | "inactive" | "error" | "awaiting_user">;

    act(() => {
      resuming = result.current({ processStopped: true });
    });
    await waitFor(() =>
      expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce(),
    );
    expect(attachLiveSessionMock).not.toHaveBeenCalled();

    await act(async () => {
      resolveDeferral?.();
      await expect(resuming).resolves.toBe("attached");
    });

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
      await result.current({ processStopped: true });
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "error",
      );
    });
    await act(async () => {
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "attached",
      );
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
      await expect(
        result.current({ abandonOnFailure: true, processStopped: true }),
      ).resolves.toBe("attached");
    });

    expect(attachLiveSessionMock).toHaveBeenCalledOnce();
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();
    consoleError.mockRestore();
  });

  test("offers saved audio after recovery attempts are exhausted", async () => {
    attachLiveSessionMock.mockRejectedValue(new Error("attach failed"));
    loadCaptureLifecycleMarkerMock.mockResolvedValue({
      version: 1,
      chunkedAudio: true,
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
      await expect(result.current({ abandonOnFailure: true })).resolves.toBe(
        "error",
      );
    });

    expect(markCaptureAudioSavedMock).toHaveBeenCalledWith("session-1");
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
  });

  test("does not offer saved audio while native capture is still running", async () => {
    attachLiveSessionMock.mockRejectedValue(new Error("attach failed"));
    getCaptureSnapshotMock.mockResolvedValue({
      status: "ok",
      data: { activeSessionId: "session-1", finalizingSessionIds: [] },
    });
    loadCaptureLifecycleMarkerMock.mockResolvedValue({
      version: 1,
      chunkedAudio: true,
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
      await expect(result.current({ abandonOnFailure: true })).resolves.toBe(
        "error",
      );
    });

    expect(markCaptureAudioSavedMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
  });

  test("keeps a stopped capture's audio for the user instead of processing it", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    loadCaptureLifecycleMarkerMock.mockResolvedValue({
      version: 1,
      chunkedAudio: true,
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
      await expect(result.current()).resolves.toBe("awaiting_user");
    });

    expect(markCaptureAudioSavedMock).toHaveBeenCalledWith("session-1");
    expect(runBatchMock).not.toHaveBeenCalled();
    expect(queueAutoEnhanceMock).not.toHaveBeenCalled();
    expect(beginCaptureRecoveryFinalizationMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
  });

  test("processes a native stopped outcome without an explicit request", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce({
        version: 1,
        chunkedAudio: true,
        retainAudio: true,
        sessionId: "session-1",
        transcriptId: "transcript-before-reload",
        startedAt: 1_000,
        createdAt: "2026-07-24T00:00:00.000Z",
        audioOffsetMs: 10_000,
        preserveExistingTranscript: true,
        ownerUserId: "user-1",
        memo: "Existing memo",
        provider: "elevenlabs",
        model: "scribe_v2",
      })
      .mockResolvedValueOnce({
        version: 1,
        chunkedAudio: true,
        retainAudio: true,
        sessionId: "session-1",
        transcriptId: "transcript-before-reload",
        startedAt: 1_000,
        createdAt: "2026-07-24T00:00:00.000Z",
        audioOffsetMs: 10_000,
        preserveExistingTranscript: true,
        ownerUserId: "user-1",
        memo: "Existing memo",
        provider: "elevenlabs",
        model: "scribe_v2",
      })
      .mockResolvedValueOnce(null);
    getStoppedCaptureMock.mockResolvedValue({
      status: "ok",
      data: {
        session_id: "session-1",
        stopped_at_ms: 123456,
        duration_seconds: 42,
        chunked_audio: true,
        audio_path: "/tmp/native-session.wav",
        requested_live_transcription: true,
        live_transcription_active: false,
        error: null,
      },
    });
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(getStoppedCaptureMock).toHaveBeenCalledWith("session-1");
    expect(acknowledgeStoppedCaptureMock).toHaveBeenCalledWith(
      "session-1",
      123456,
    );
    expect(runBatchMock).toHaveBeenCalledWith("/tmp/existing-session.mp3", {
      deferAudioFinalization: true,
      notifyOnCompletion: true,
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

  test("falls back to whole-chunk recovery when the native ledger starts too late", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    const marker = {
      version: 1 as const,
      chunkedAudio: true,
      retainAudio: true,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 0,
      preserveExistingTranscript: false,
      ownerUserId: "user-1",
      memo: "",
      provider: "anarlog",
      model: "am-test",
    };
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    getStoppedCaptureMock.mockResolvedValue({
      status: "ok",
      data: {
        session_id: "session-1",
        stopped_at_ms: 123_456,
        duration_seconds: 60,
        chunked_audio: true,
        audio_path: null,
        requested_live_transcription: true,
        live_transcription_active: false,
        error: null,
      },
    });
    vi.mocked(transcriptionCommands.getCaptureAudioGaps).mockResolvedValue({
      status: "ok",
      data: {
        capture_started_at_ms: 61_001,
        gaps: [{ start_ms: 21_000, end_ms: 31_000 }],
        open_gap_started_at_ms: null,
        awaiting_connection: false,
        storage_failed: false,
        confirmed_through_ms: null,
      },
    });
    vi.mocked(transcriptionCommands.listCaptureAudioChunks).mockResolvedValue({
      status: "ok",
      data: [
        {
          id: "1000-0-60000-0.mp3",
          path: "/tmp/recovery-chunk.mp3",
          capture_started_at: 1_000,
          start_ms: 0,
          audio_start_ms: 0,
          end_ms: 60_000,
        },
      ],
    });
    runBatchMock.mockImplementationOnce(async (_path, options) => {
      await options.recovery.persist(
        [
          {
            id: "recovered-word",
            text: "recovered",
            start_ms: 0,
            end_ms: 1,
            channel: 0,
          },
        ],
        [],
      );
    });
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(appendRecoveredTranscriptWordsMock).toHaveBeenCalledWith(
      "transcript-before-reload",
      [
        {
          id: "recovered-word",
          text: "recovered",
          start_ms: 0,
          end_ms: 1,
          channel: 0,
        },
      ],
      [],
      [{ start: 0, end: 60_000 }],
      [],
    );
  });

  test("rechecks for a native stopped outcome after attaching listeners", async () => {
    attachLiveSessionMock.mockResolvedValue("inactive");
    const marker = {
      version: 1 as const,
      chunkedAudio: true,
      retainAudio: true,
      sessionId: "session-1",
      transcriptId: "transcript-before-reload",
      startedAt: 1_000,
      createdAt: "2026-07-24T00:00:00.000Z",
      audioOffsetMs: 10_000,
      preserveExistingTranscript: true,
      ownerUserId: "user-1",
      memo: "Existing memo",
      provider: "elevenlabs",
      model: "scribe_v2",
    };
    loadCaptureLifecycleMarkerMock
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(marker)
      .mockResolvedValueOnce(null);
    getStoppedCaptureMock
      .mockResolvedValueOnce({ status: "ok", data: null })
      .mockResolvedValueOnce({
        status: "ok",
        data: {
          session_id: "session-1",
          stopped_at_ms: 123456,
          duration_seconds: 42,
          chunked_audio: true,
          audio_path: "/tmp/native-session.wav",
          requested_live_transcription: true,
          live_transcription_active: false,
          error: null,
        },
      });
    const { result } = renderHook(() =>
      useResumeListeningLifecycle("session-1"),
    );

    await act(async () => {
      await expect(result.current()).resolves.toBe("inactive");
    });

    expect(getStoppedCaptureMock).toHaveBeenCalledTimes(2);
    expect(acknowledgeStoppedCaptureMock).toHaveBeenCalledWith(
      "session-1",
      123456,
    );
  });

  test.each(["forever", "none"] as const)(
    "transcribes a Scribe V2 capture with %s retention after a stopped renderer recovers",
    async (retention) => {
      useConfigValueMock.mockImplementation((key: string) =>
        key === "audio_retention" ? retention : undefined,
      );
      attachLiveSessionMock.mockResolvedValue("inactive");
      const marker = {
        version: 1 as const,
        chunkedAudio: true,
        retainAudio: true,
        sessionId: "session-1",
        transcriptId: "transcript-before-reload",
        startedAt: 1_000,
        createdAt: "2026-07-24T00:00:00.000Z",
        audioOffsetMs: 0,
        preserveExistingTranscript: false,
        ownerUserId: "user-1",
        memo: "",
        provider: "elevenlabs",
        model: "scribe_v2",
      };
      loadCaptureLifecycleMarkerMock
        .mockResolvedValueOnce(marker)
        .mockResolvedValueOnce(marker)
        .mockResolvedValueOnce(null);
      const { result } = renderHook(() =>
        useResumeListeningLifecycle("session-1"),
      );

      await act(async () => {
        await expect(result.current({ processStopped: true })).resolves.toBe(
          "inactive",
        );
      });

      expect(runBatchMock).toHaveBeenCalledWith("/tmp/existing-session.mp3", {
        deferAudioFinalization: true,
        notifyOnCompletion: true,
        promotion: { scope: "whole_session" },
      });
      if (retention === "none") {
        expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledWith(
          "none",
          "session-1",
        );
        expect(runBatchMock.mock.invocationCallOrder[0]!).toBeLessThan(
          deleteProcessedAudioForRetentionMock.mock.invocationCallOrder[0]!,
        );
      }
    },
  );

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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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

  test("shows the stored start error when native capture rejects the start", async () => {
    startMock.mockResolvedValueOnce(false);
    getLiveStartErrorMock.mockReturnValueOnce("session already running");
    const { result } = renderHook(() => useStartListening("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(getLiveStartErrorMock).toHaveBeenCalledWith("session-1");
    expect(toastErrorMock).toHaveBeenCalledWith(
      "Another recording is still running",
      expect.objectContaining({ id: "capture-start-failed" }),
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "error",
      );
    });
    expect(toastErrorMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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
      await expect(
        result.current({ abandonOnFailure: true, processStopped: true }),
      ).resolves.toBe("error");
    });
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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
      await expect(
        result.current({ abandonOnFailure: true, processStopped: true }),
      ).resolves.toBe("error");
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
      await expect(
        recoveryResult.result.current({ processStopped: true }),
      ).resolves.toBe("inactive");
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "error",
      );
    });
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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
      await expect(result.current({ processStopped: true })).rejects.toThrow(
        "database is locked",
      );
    });
    await act(async () => {
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "error",
      );
    });
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(runBatchMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "error",
      );
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "error",
      );
    });
    expect(beginCloudsyncActivityMock).toHaveBeenCalledOnce();
    expect(endCloudsyncActivityMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "error",
      );
    });

    expect(runBatchMock).not.toHaveBeenCalled();
    expect(finishCaptureRecoveryFinalizationMock).not.toHaveBeenCalled();

    await act(async () => {
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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
      await expect(
        result.current({ abandonOnFailure: true, processStopped: true }),
      ).resolves.toBe("error");
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "error",
      );
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
      await expect(result.current({ processStopped: true })).resolves.toBe(
        "inactive",
      );
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

  test("repairs from finalized audio when live transcript persistence fails", async () => {
    failNextNativePersistenceFlush();
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
      notifyOnCompletion: false,
      promotion: { scope: "whole_session" },
    });
    expect(toastErrorMock).toHaveBeenCalledWith(
      "Your transcript could not be saved",
      expect.anything(),
    );
    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
      "session-1",
    );
    consoleError.mockRestore();
  });

  describe("summaries during post-stop repair", () => {
    async function startWithLiveTranscript() {
      const { result } = renderHook(() => useStartListening("session-1"));
      await act(async () => {
        await result.current();
      });
      const callbacks = startMock.mock.calls[0]?.[1];
      callbacks.handlePersist({
        new_words: [
          {
            id: "word-1",
            text: "Meeting notes",
            start_ms: 0,
            end_ms: 100,
            channel: 0,
          },
        ],
        replaced_ids: [],
        partials: [],
      });
      return callbacks;
    }

    const stoppedDetails = {
      durationSeconds: 42,
      audioPath: "/tmp/session.wav",
      requestedLiveTranscription: true,
      liveTranscriptionActive: true,
      needsBatchRepair: true,
    };

    test.each([
      {
        reason: "an incomplete stream",
        liveTranscriptionActive: true,
        needsBatchRepair: true,
      },
      {
        reason: "a disconnected stream",
        liveTranscriptionActive: false,
        needsBatchRepair: true,
      },
      {
        reason: "speaker refinement",
        liveTranscriptionActive: true,
        needsBatchRepair: false,
      },
    ])("waits for repair of $reason before summarizing", async (details) => {
      useSTTConnectionMock.mockReturnValue({
        conn: {
          provider: "anarlog",
          model: "cloud",
          baseUrl: "https://api.anarlog.so/stt",
          apiKey: "test",
        },
      });
      useSessionParticipantHumanIdsMock.mockReturnValue([
        "user-1",
        "speaker-1",
        "speaker-2",
      ]);
      let finishBatch: (() => void) | undefined;
      runBatchMock.mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            finishBatch = resolve;
          }),
      );
      const callbacks = await startWithLiveTranscript();
      const stopped = callbacks.onStopped("session-1", {
        ...stoppedDetails,
        ...details,
      });

      await waitFor(() => expect(runBatchMock).toHaveBeenCalledOnce());
      expect(requestAutoEnhanceMock).not.toHaveBeenCalled();
      expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();

      finishBatch?.();
      await act(async () => await stopped);

      expect(requestAutoEnhanceMock.mock.calls).toEqual([
        ["session-1", "refresh"],
      ]);
      expect(saveCaptureLifecycleMarkerMock).toHaveBeenLastCalledWith(
        expect.objectContaining({
          summaryMode: "refresh",
        }),
      );
      expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledOnce();
    });

    test.each(["upload failed", "Transcription stopped."])(
      "does not summarize when repair ends with %s",
      async (message) => {
        const consoleError = vi
          .spyOn(console, "error")
          .mockImplementation(() => {});
        runBatchMock.mockRejectedValueOnce(new Error(message));
        const callbacks = await startWithLiveTranscript();

        await act(
          async () => await callbacks.onStopped("session-1", stoppedDetails),
        );

        expect(requestAutoEnhanceMock).not.toHaveBeenCalled();
        expect(resetEnhanceTasksMock).not.toHaveBeenCalled();
        expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
        if (message === "Transcription stopped.") {
          expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledOnce();
          expect(requestCaptureRecoveryMock).not.toHaveBeenCalled();
        } else {
          expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
          expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
        }
        consoleError.mockRestore();
      },
    );

    test("refreshes the early summary after repair recovers across a reload", async () => {
      attachLiveSessionMock.mockResolvedValue("inactive");
      transcriptExistsMock.mockResolvedValue(true);
      const marker = {
        version: 1 as const,
        sessionId: "session-1",
        transcriptId: "transcript-before-reload",
        startedAt: 1_000,
        createdAt: "2026-07-24T00:00:00.000Z",
        audioOffsetMs: 0,
        preserveExistingTranscript: false,
        ownerUserId: "user-1",
        memo: "Existing memo",
        refreshSummaryAfterRepair: true,
      };
      loadCaptureLifecycleMarkerMock
        .mockResolvedValueOnce(marker)
        .mockResolvedValueOnce(marker)
        .mockResolvedValueOnce(null);
      const { result } = renderHook(() =>
        useResumeListeningLifecycle("session-1"),
      );

      await act(async () => {
        await expect(result.current({ processStopped: true })).resolves.toBe(
          "inactive",
        );
      });

      expect(runBatchMock).toHaveBeenCalledOnce();
      expect(runBatchMock).toHaveBeenCalledBefore(requestAutoEnhanceMock);
      expect(requestAutoEnhanceMock.mock.calls).toEqual([
        ["session-1", "regenerate"],
      ]);
      expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledOnce();
    });
  });

  test("does not summarize an incomplete live transcript without repair audio", async () => {
    failNextNativePersistenceFlush();
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

    expect(toastErrorMock).toHaveBeenCalledWith(
      "Anarlog could not save part of the live transcript.",
      { id: "live-transcript-persist-failed" },
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
    failNextNativePersistenceFlush();
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

    expect(toastErrorMock).toHaveBeenCalledWith(
      "Anarlog could not finish saving the transcript. The recording was kept so you can try again.",
      { id: "post-capture-transcript-incomplete" },
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

  test("ends automatic recovery after the user cancels the batch repair", async () => {
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
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(saveCaptureLifecycleMarkerMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ phase: "finalizing" }),
    );
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      "generated-id",
    );
    expect(endCloudsyncActivityMock).toHaveBeenCalledWith(
      "capture",
      "session-1:generated-id",
    );
    expect(requestCaptureRecoveryMock).not.toHaveBeenCalled();
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(softDeleteTranscriptMock).not.toHaveBeenCalled();
    expect(setBatchTranscriptionPendingMock).toHaveBeenLastCalledWith(
      "session-1",
      false,
    );
  });

  test("does not restart cancelled recovery after reattaching or reloading", async () => {
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
    let pendingMarker: typeof marker | null = marker;
    loadCaptureLifecycleMarkerMock.mockImplementation(
      async () => pendingMarker,
    );
    clearCaptureLifecycleMarkerMock.mockImplementation(async () => {
      pendingMarker = null;
    });
    runBatchMock.mockRejectedValueOnce(new Error("Transcription stopped."));
    const recovery = renderHook(() => useResumeListeningLifecycle("session-1"));

    await act(async () => {
      await expect(
        recovery.result.current({ processStopped: true }),
      ).resolves.toBe("inactive");
      await expect(
        recovery.result.current({ processStopped: true }),
      ).resolves.toBe("inactive");
    });
    recovery.unmount();
    const reloaded = renderHook(() => useResumeListeningLifecycle("session-1"));
    await act(async () => {
      await expect(
        reloaded.result.current({ processStopped: true }),
      ).resolves.toBe("inactive");
    });

    expect(runBatchMock).toHaveBeenCalledOnce();
    expect(clearCaptureLifecycleMarkerMock).toHaveBeenCalledWith(
      "session-1",
      marker.transcriptId,
    );
    expect(finishCaptureRecoveryFinalizationMock).toHaveBeenCalledOnce();
    expect(requestCaptureRecoveryMock).not.toHaveBeenCalled();
    expect(requestAutoEnhanceMock).not.toHaveBeenCalled();
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(softDeleteTranscriptMock).not.toHaveBeenCalled();
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

  test("regenerates the summary after resumed live capture is journaled", async () => {
    succeedNextNativePersistenceFlush();
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
    await act(async () => await stopped);

    expect(resetEnhanceTasksMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceMock).toHaveBeenCalledWith("session-1");
    expect(queueAutoEnhanceIfSummaryEmptyMock).not.toHaveBeenCalled();
  });

  test("requests recovery when the deleted session is restored during finalization", async () => {
    useSessionHasTranscriptMock.mockReturnValue(true);
    isSessionDeletedMock
      .mockResolvedValueOnce(true)
      .mockResolvedValueOnce(false);
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

    expect(isSessionDeletedMock).toHaveBeenCalledTimes(2);
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(clearCaptureLifecycleMarkerMock).not.toHaveBeenCalled();
    expect(requestCaptureRecoveryMock).toHaveBeenCalledWith("session-1");
    consoleError.mockRestore();
  });

  test("replaces only the current live transcript when resumed capture needs batch repair", async () => {
    succeedNextNativePersistenceFlush();
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
      notifyOnCompletion: false,
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
    expect(toastWarningMock).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        id: "recording-with-limited-transcription-languages",
        action: expect.anything(),
      }),
    );

    const warningCalls = toastWarningMock.mock.calls;
    const warningOptions = warningCalls[warningCalls.length - 1]?.[1];
    warningOptions?.action.onClick();

    expect(openNewMock).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "transcription" },
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

  test("retries until a conferencing app is mic-active without reporting intermediate failures", async () => {
    listMicUsingApplicationsMock
      .mockResolvedValueOnce({
        status: "ok",
        data: [{ id: "com.anarlog.dev", name: "Anarlog Dev" }],
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: [{ id: "us.zoom.xos", name: "zoom.us" }],
      });
    sendMeetingChatMessageMock
      .mockResolvedValueOnce({
        status: "ok",
        data: {
          sent: false,
          platform: "unknown",
          surface: "unknown",
          warnings: [
            "refusing to send because the mic-active apps contain 0 recognized meeting app bundles; expected exactly one",
          ],
        },
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: { sent: true, platform: "zoom", surface: "native", warnings: [] },
      });

    await expect(
      sendMeetingRecordingDisclosure({
        maxAttempts: 2,
        retryIntervalMs: 0,
      }),
    ).resolves.toEqual({ status: "sent" });

    expect(listMicUsingApplicationsMock).toHaveBeenCalledTimes(2);
    expect(sendMeetingChatMessageMock).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining("https://anarlog.so"),
      ["com.anarlog.dev"],
    );
    expect(sendMeetingChatMessageMock).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining("https://anarlog.so"),
      ["us.zoom.xos"],
    );
    expect(toastWarningMock).not.toHaveBeenCalled();
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
    expect(toastWarningMock).toHaveBeenCalledTimes(1);
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
    expect(toastWarningMock).not.toHaveBeenCalled();
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
        onParticipantDeclined: expect.any(Function),
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
        onParticipantDeclined: expect.any(Function),
      });
    });

    expect(listMicUsingApplicationsMock).not.toHaveBeenCalled();
  });
});
