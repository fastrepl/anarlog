import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { beginCloudsyncActivity, endCloudsyncActivity } from "@anlg/plugin-db";

import { BatchResponseProcessingError } from "./batch-response-processing-error";
import {
  canRunBatchTranscription,
  EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE,
  getBatchFallbackTarget,
  getBatchProvider,
  getSessionSpeakerCount,
  isTerminalTranscriptionError,
} from "./useRunBatch";
import { useRunBatch } from "./useRunBatch";

const {
  startTranscriptionMock,
  stopTranscriptionMock,
  useListenerMock,
  useSessionMock,
  useSessionParticipantsMock,
  useSTTConnectionMock,
  useAuthMock,
  getSessionForRequestMock,
  refreshSessionMock,
  useBillingAccessMock,
  useConfigValueMock,
  isSupportedLanguagesBatchMock,
  toastWarningMock,
  deleteProcessedAudioForRetentionMock,
  saveBatchTranscriptMock,
  reconcileRefinedSpeakerClustersMock,
  maybeExtractVoiceprintCandidatesMock,
  notifyBatchCompletedMock,
  idMock,
  archMock,
  platformMock,
} = vi.hoisted(() => ({
  startTranscriptionMock: vi.fn(),
  stopTranscriptionMock: vi.fn(),
  useListenerMock: vi.fn(),
  useSessionMock: vi.fn(),
  useSessionParticipantsMock: vi.fn(),
  useSTTConnectionMock: vi.fn(),
  useAuthMock: vi.fn(),
  getSessionForRequestMock: vi.fn(),
  refreshSessionMock: vi.fn(),
  useBillingAccessMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  isSupportedLanguagesBatchMock: vi.fn(),
  toastWarningMock: vi.fn(),
  deleteProcessedAudioForRetentionMock: vi.fn(),
  saveBatchTranscriptMock: vi.fn(),
  reconcileRefinedSpeakerClustersMock: vi.fn(),
  maybeExtractVoiceprintCandidatesMock: vi.fn(),
  notifyBatchCompletedMock: vi.fn(),
  idMock: vi.fn(),
  archMock: vi.fn(),
  platformMock: vi.fn(),
}));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    saveBatchTranscript: saveBatchTranscriptMock,
    reconcileRefinedSpeakerClusters: reconcileRefinedSpeakerClustersMock,
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  arch: archMock,
  platform: platformMock,
}));

vi.mock("./contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("./useKeywords", () => ({
  getSessionKeywords: vi.fn(async () => []),
  useKeywords: vi.fn(() => []),
}));

vi.mock("./useSTTConnection", () => ({
  useSTTConnection: useSTTConnectionMock,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: {
    warning: toastWarningMock,
  },
}));

vi.mock("~/auth", () => ({
  useAuth: useAuthMock,
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: useBillingAccessMock,
}));

vi.mock("~/env", () => ({
  env: {
    VITE_API_URL: "https://api.test",
  },
}));

vi.mock("~/services/audio-retention", () => ({
  deleteProcessedAudioForRetention: deleteProcessedAudioForRetentionMock,
  normalizeAudioRetention: (value: unknown) =>
    typeof value === "string" ? value : "forever",
}));

vi.mock("~/services/voiceprint", () => ({
  maybeExtractVoiceprintCandidates: maybeExtractVoiceprintCandidatesMock,
}));

vi.mock("~/session/queries", () => ({
  useSession: useSessionMock,
  useSessionParticipants: useSessionParticipantsMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/shared/utils", () => ({
  id: idMock,
}));

vi.mock("~/stt/capabilities", () => {
  const baseLanguageCode = (language: string) =>
    language.split(/[-_]/)[0]?.toLowerCase() ?? "";

  return {
    getTranscriptionLanguages: (
      mainLanguage: string | null | undefined,
      spokenLanguages: readonly string[] | null | undefined,
    ) => {
      const seen = new Set<string>();
      const languages: string[] = [];

      for (const language of [mainLanguage, ...(spokenLanguages ?? [])]) {
        if (!language) {
          continue;
        }

        const baseCode = baseLanguageCode(language);
        if (!baseCode || seen.has(baseCode)) {
          continue;
        }

        seen.add(baseCode);
        languages.push(language);
      }

      return languages;
    },
    isDesktopLocalSttAvailable: (
      currentPlatform: string,
      currentArch: string,
    ) => currentPlatform === "macos" && currentArch === "aarch64",
    isLocalFileSttModel: (
      provider: string | null | undefined,
      model: string | null | undefined,
    ) => provider === "local_file" && model === "local-file",
    isOnDeviceSttModel: (
      provider: string | null | undefined,
      model: string | null | undefined,
    ) =>
      typeof model === "string" &&
      ((provider === "soniqo" && model.startsWith("soniqo-")) ||
        (provider === "apple_speech" && model === "apple-speech") ||
        (provider === "anarlog" &&
          (model.startsWith("soniqo-") ||
            model.startsWith("am-") ||
            model.startsWith("Quantized")))),
    isSupportedLanguagesBatch: isSupportedLanguagesBatchMock,
  };
});

vi.mock("~/store/zustand/listener/general-batch", () => ({
  acknowledgeCompletedBatch: vi.fn(async () => {}),
  notifyBatchCompleted: notifyBatchCompletedMock,
}));

describe("getBatchProvider", () => {
  test.each([
    ["pyannote", "parakeet-tdt-0.6b-v3", "pyannote"],
    ["openai", "gpt-4o-transcribe", "openai"],
    ["cartesia", "ink-2", "cartesia"],
    ["cohere", "cohere-transcribe-03-2026", "cohere"],
    ["mistral", "voxtral-mini-2602", "mistral"],
    ["aws_transcribe", "amazon-transcribe", "aws_transcribe"],
    ["azure_speech", "fast-transcription", "azure_speech"],
    ["google_cloud", "latest_long", "google_cloud"],
    ["google_generative_ai", "gemini-3.5-transcribe", "google_generative_ai"],
    ["groq", "whisper-large-v3-turbo", "groq"],
    ["openrouter", "openai/gpt-4o-mini-transcribe", "openrouter"],
    ["siliconflow", "FunAudioLLM/SenseVoiceSmall", "siliconflow"],
    ["zai", "glm-asr-2512", "zai"],
    ["revai", "machine", "revai"],
    ["speechmatics", "enhanced", "speechmatics"],
    ["together", "openai/whisper-large-v3", "together"],
    ["xai", "xai-stt", "xai"],
    ["smallestai", "pulse", "smallestai"],
    ["meta", "muse-voice-transcribe-1.0", "meta"],
    ["cloudflare_workers_ai", "nova-3", "deepgram"],
    ["custom", "nova-3", "deepgram"],
    ["anarlog", "soniqo-parakeet-batch", "soniqo"],
    ["soniqo", "soniqo-parakeet-batch", "soniqo"],
    ["apple_speech", "apple-speech", "applespeech"],
    ["local_file", "local-file", "whispercpp"],
  ] as const)("maps %s/%s to %s", (provider, model, expected) => {
    expect(getBatchProvider(provider, model)).toBe(expected);
  });
});

describe("canRunBatchTranscription", () => {
  test("allows post-capture batch so useRunBatch can choose a fallback", () => {
    expect(canRunBatchTranscription(null)).toBe(true);
    expect(
      canRunBatchTranscription({
        provider: "custom",
        model: "realtime-only",
      }),
    ).toBe(true);
  });
});

describe("isTerminalTranscriptionError", () => {
  test("stops retries after a provider response cannot be processed", () => {
    expect(
      isTerminalTranscriptionError(
        new BatchResponseProcessingError(new Error("database is locked")),
      ),
    ).toBe(true);
  });

  test.each([
    "Bad Request: failed to process audio: corrupt or unsupported data",
    "No speech detected",
    "Authentication failed: 401 Unauthorized",
    EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE,
  ])("classifies permanent failures: %s", (message) => {
    expect(isTerminalTranscriptionError(new Error(message))).toBe(true);
  });

  test.each([
    "request timed out",
    "429 Too Many Requests",
    "503 Service Unavailable",
    "database is locked",
    "nova-3 is not available for batch transcription",
    "STT connection is not available",
  ])("leaves transient failures retryable: %s", (message) => {
    expect(isTerminalTranscriptionError(new Error(message))).toBe(false);
  });
});

describe("getBatchFallbackTarget", () => {
  test("uses hosted cloud transcription for paid users with a session", () => {
    expect(
      getBatchFallbackTarget({
        isPaid: true,
        accessToken: "token",
        apiBaseUrl: "https://api.test",
        currentPlatform: "windows",
        currentArch: "x86_64",
      }),
    ).toEqual({
      provider: "anarlog",
      model: "cloud",
      baseUrl: "https://api.test/stt",
      apiKey: "token",
      label: "Pro cloud transcription",
    });
  });

  test("uses local Soniqo batch transcription otherwise", () => {
    expect(
      getBatchFallbackTarget({
        isPaid: false,
        accessToken: null,
        apiBaseUrl: "https://api.test",
        currentPlatform: "macos",
        currentArch: "aarch64",
      }),
    ).toEqual({
      provider: "soniqo",
      model: "soniqo-parakeet-batch",
      baseUrl: "soniqo://local",
      apiKey: "",
      label: "Soniqo batch transcription",
    });
  });

  test.each([
    { currentPlatform: "windows" as const, currentArch: "x86_64" as const },
    { currentPlatform: "linux" as const, currentArch: "x86_64" as const },
    { currentPlatform: "macos" as const, currentArch: "x86_64" as const },
  ])(
    "does not use local Soniqo on $currentPlatform/$currentArch",
    ({ currentPlatform, currentArch }) => {
      expect(
        getBatchFallbackTarget({
          isPaid: false,
          accessToken: null,
          apiBaseUrl: "https://api.test",
          currentPlatform,
          currentArch,
        }),
      ).toBeNull();
    },
  );
});

describe("useRunBatch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    saveBatchTranscriptMock.mockImplementation(async (request) => ({
      status: "ok",
      data: {
        status: "saved",
        transcript_id: request.transcript_id,
      },
    }));
    stopTranscriptionMock.mockResolvedValue(undefined);
    archMock.mockReturnValue("aarch64");
    platformMock.mockReturnValue("macos");

    let nextId = 0;
    idMock.mockImplementation(() => `generated-${++nextId}`);
    notifyBatchCompletedMock.mockResolvedValue(undefined);
    deleteProcessedAudioForRetentionMock.mockResolvedValue(undefined);
    isSupportedLanguagesBatchMock.mockResolvedValue(true);
    useListenerMock.mockImplementation((selector) =>
      selector({
        startTranscription: startTranscriptionMock,
        stopTranscription: stopTranscriptionMock,
      }),
    );
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "Existing memo",
    });
    useSessionParticipantsMock.mockReturnValue([]);
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "deepgram",
        model: "nova-3",
        baseUrl: "https://api.deepgram.com/v1/listen",
        apiKey: "test-key",
      },
    });
    useAuthMock.mockReturnValue({
      session: {
        access_token: "paid-token",
        user: { id: "user-1" },
      },
      getSessionForRequest: getSessionForRequestMock,
      refreshSession: refreshSessionMock,
    });
    getSessionForRequestMock.mockResolvedValue({
      access_token: "paid-token",
    });
    refreshSessionMock.mockResolvedValue(null);
    useBillingAccessMock.mockReturnValue({
      isPaid: false,
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "en" : [],
    );
  });

  test.each([
    { name: "a pre-aborted signal", cancelsDuringAuthPreflight: false },
    { name: "a cancelled auth preflight", cancelsDuringAuthPreflight: true },
  ])(
    "does not start a dictation transcription after $name",
    async ({ cancelsDuringAuthPreflight }) => {
      const abort = new AbortController();
      let finish: ((value: null) => void) | undefined;
      if (cancelsDuringAuthPreflight) {
        useBillingAccessMock.mockReturnValue({ isPaid: true });
        getSessionForRequestMock.mockReturnValueOnce(
          new Promise((resolve) => {
            finish = resolve;
          }),
        );
      } else {
        abort.abort();
      }
      const { result } = renderHook(() => useRunBatch("dictation"));
      const run = result.current("/tmp/voice.wav", { signal: abort.signal });
      const rejected = expect(run).rejects.toMatchObject({
        name: "AbortError",
      });
      if (cancelsDuringAuthPreflight) {
        await waitFor(() =>
          expect(getSessionForRequestMock).toHaveBeenCalled(),
        );
        abort.abort();
        finish?.(null);
      }
      await rejected;
      expect(startTranscriptionMock).not.toHaveBeenCalled();
    },
  );

  test("cancels the active provider and never retries authentication after abort", async () => {
    const abort = new AbortController();
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
        model: "cloud",
        baseUrl: "https://api.test/stt",
        apiKey: "stale",
      },
    });
    let fail!: (reason: Error) => void;
    startTranscriptionMock.mockImplementationOnce(
      () =>
        new Promise<void>((_resolve, reject) => {
          fail = reject;
        }),
    );
    const { result } = renderHook(() => useRunBatch("dictation"));
    const run = result.current("/tmp/voice.wav", { signal: abort.signal });
    const rejected = expect(run).rejects.toMatchObject({ name: "AbortError" });
    await waitFor(() => expect(startTranscriptionMock).toHaveBeenCalledOnce());
    abort.abort();
    const persist = startTranscriptionMock.mock.calls[0]?.[1]?.handlePersist;
    expect(() =>
      persist?.(
        [{ text: "cancelled", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      ),
    ).not.toThrow();
    fail(
      new Error(
        "Authentication failed. Please check your API key in settings.",
      ),
    );
    await rejected;
    expect(stopTranscriptionMock).toHaveBeenCalledWith("dictation");
    expect(refreshSessionMock).not.toHaveBeenCalled();
    expect(startTranscriptionMock).toHaveBeenCalledOnce();
    expect(saveBatchTranscriptMock).not.toHaveBeenCalled();
  });

  test("retries cancellation after native startup finishes", async () => {
    const abort = new AbortController();
    let started!: () => void;
    startTranscriptionMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          started = resolve;
        }),
    );
    const { result } = renderHook(() => useRunBatch("dictation"));
    const run = result.current("/tmp/voice.wav", { signal: abort.signal });
    const rejected = expect(run).rejects.toMatchObject({ name: "AbortError" });
    await waitFor(() => expect(startTranscriptionMock).toHaveBeenCalledOnce());
    abort.abort();
    expect(stopTranscriptionMock).not.toHaveBeenCalled();
    started();
    await rejected;
    expect(stopTranscriptionMock).toHaveBeenCalledTimes(1);
  });

  test("promotes the complete streamed transcript before retention", async () => {
    let finishTranscription: (() => void) | undefined;
    startTranscriptionMock.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finishTranscription = resolve;
        }),
    );

    const { result } = renderHook(() => useRunBatch("session-1"));
    const run = result.current("/tmp/session.wav", {
      promotion: { scope: "whole_session" },
    });

    await waitFor(() => {
      expect(startTranscriptionMock).toHaveBeenCalledTimes(1);
    });
    const persist = startTranscriptionMock.mock.calls[0]?.[1]?.handlePersist;
    persist?.([{ text: "hello", start_ms: 0, end_ms: 100, channel: 0 }], []);
    persist?.([{ text: "world", start_ms: 100, end_ms: 200, channel: 0 }], []);

    expect(saveBatchTranscriptMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    expect(notifyBatchCompletedMock).not.toHaveBeenCalled();

    finishTranscription?.();
    await act(async () => await run);

    expect(beginCloudsyncActivity).toHaveBeenCalledWith(
      "transcription",
      "session-1:generated-1",
    );
    expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        session_id: "session-1",
        transcript_id: expect.any(String),
        owner_user_id: "user-1",
        created_at: expect.any(String),
        started_at: expect.any(Number),
        memo: "Existing memo",
        provider: "deepgram",
        model: "nova-3",
        promotion: { scope: "whole_session" },
        mark_audio_complete: true,
        words: [
          expect.objectContaining({ text: "hello" }),
          expect.objectContaining({ text: "world" }),
        ],
        hints: [],
      }),
    );
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledTimes(1);
    expect(startTranscriptionMock.mock.calls[0]?.[1]?.notifyOnCompletion).toBe(
      false,
    );
    expect(notifyBatchCompletedMock).toHaveBeenCalledWith("session-1");
    expect(saveBatchTranscriptMock.mock.invocationCallOrder[0]).toBeLessThan(
      notifyBatchCompletedMock.mock.invocationCallOrder[0],
    );
    expect(saveBatchTranscriptMock.mock.invocationCallOrder[0]).toBeLessThan(
      deleteProcessedAudioForRetentionMock.mock.invocationCallOrder[0],
    );
    expect(
      deleteProcessedAudioForRetentionMock.mock.invocationCallOrder[0],
    ).toBeLessThan(
      vi.mocked(endCloudsyncActivity).mock.invocationCallOrder[0]!,
    );
  });

  test("defers audio finalization for capture recovery", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "recovered", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", {
        deferAudioFinalization: true,
        promotion: { scope: "whole_session" },
      });
    });

    expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ mark_audio_complete: false }),
    );
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("repairs a chunk separately from live capture and waits for its database commit", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "recovered", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });
    let commit!: () => void;
    const persist = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          commit = resolve;
        }),
    );
    const { result } = renderHook(() => useRunBatch("session-1"));
    let completed = false;
    const run = result
      .current("/tmp/chunk.mp3", { recovery: { persist } })
      .then(() => {
        completed = true;
      });
    await waitFor(() => expect(persist).toHaveBeenCalledOnce());
    expect(startTranscriptionMock.mock.calls[0]?.[0]).toMatchObject({
      session_id: "session-1:recovery",
    });
    expect(startTranscriptionMock.mock.calls[0]?.[1]).toMatchObject({
      recovery: true,
    });
    expect(completed).toBe(false);
    expect(saveBatchTranscriptMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
    commit();
    await act(async () => await run);
    expect(completed).toBe(true);
  });

  test("cancels only the background repair when its capture ends", async () => {
    const abort = new AbortController();
    let finish!: () => void;
    startTranscriptionMock.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const persist = vi.fn();
    const { result } = renderHook(() => useRunBatch("session-1"));
    const run = result.current("/tmp/chunk.mp3", {
      signal: abort.signal,
      recovery: { persist },
    });
    const rejected = expect(run).rejects.toMatchObject({ name: "AbortError" });
    await waitFor(() => expect(startTranscriptionMock).toHaveBeenCalledOnce());
    abort.abort();
    finish();
    await rejected;
    expect(stopTranscriptionMock).toHaveBeenCalledWith("session-1:recovery");
    expect(stopTranscriptionMock).not.toHaveBeenCalledWith("session-1");
    expect(persist).not.toHaveBeenCalled();
  });

  test("does not make completed provider work retryable when persistence fails", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "recovered", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });
    saveBatchTranscriptMock.mockRejectedValueOnce(
      new Error("disk write failed"),
    );

    const { result } = renderHook(() => useRunBatch("session-1"));
    let processingError: unknown;

    await act(async () => {
      try {
        await result.current("/tmp/session.wav", {
          deferAudioFinalization: true,
          promotion: { scope: "whole_session" },
        });
      } catch (error) {
        processingError = error;
      }
    });

    expect(processingError).toBeInstanceOf(BatchResponseProcessingError);
    expect(isTerminalTranscriptionError(processingError)).toBe(true);
    expect(startTranscriptionMock).toHaveBeenCalledOnce();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("does not save for custom batch persist handlers", async () => {
    const handlePersist = vi.fn();
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "custom", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", { handlePersist });
    });

    expect(handlePersist).toHaveBeenCalledTimes(1);
    expect(saveBatchTranscriptMock).not.toHaveBeenCalled();
  });

  test("promotes post-stop batch by replacing the live current capture", async () => {
    useConfigValueMock.mockImplementation((key) =>
      key === "remember_speakers" ? true : key === "ai_language" ? "en" : [],
    );
    saveBatchTranscriptMock.mockResolvedValueOnce({
      status: "ok",
      data: {
        status: "saved",
        transcript_id: "saved-transcript-id",
      },
    });
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [
          { text: "old", start_ms: 10_000, end_ms: 10_500, channel: 0 },
          { text: "new", start_ms: 60_100, end_ms: 60_500, channel: 0 },
        ],
        [
          {
            wordIndex: 0,
            data: {
              type: "provider_speaker_index",
              speaker_index: 0,
            },
          },
          {
            wordIndex: 1,
            data: {
              type: "provider_speaker_index",
              speaker_index: 1,
            },
          },
        ],
        { mode: "replace" },
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", {
        promotion: {
          scope: "current_capture",
          audioOffsetMs: 60_000,
          replaceTranscriptId: "transcript-current-live",
          startedAt: 123_000,
        },
      });
    });

    expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        session_id: "session-1",
        transcript_id: expect.any(String),
        promotion: {
          scope: "current_capture",
          audio_offset_ms: 60_000,
          replace_transcript_id: "transcript-current-live",
          started_at: 123_000,
        },
        mark_audio_complete: true,
        words: expect.arrayContaining([
          expect.objectContaining({ text: "old", start_ms: 10_000 }),
          expect.objectContaining({ text: "new", start_ms: 60_100 }),
        ]),
        hints: expect.arrayContaining([
          expect.objectContaining({ type: "provider_speaker_index" }),
        ]),
      }),
    );
    expect(maybeExtractVoiceprintCandidatesMock).toHaveBeenCalledWith(
      expect.objectContaining({
        enabled: true,
        sessionId: "session-1",
        transcriptId: "saved-transcript-id",
        audioPath: "/tmp/session.wav",
      }),
    );
  });

  test("passes staged speaker hints to the atomic save command", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [
          {
            text: "answer",
            start_ms: 60_100,
            end_ms: 60_500,
            channel: 1,
          },
        ],
        [
          {
            wordIndex: 0,
            data: {
              type: "provider_speaker_index",
              channel: 1,
              speaker_index: 3,
            },
          },
        ],
        { mode: "replace" },
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));
    await act(async () => {
      await result.current("/tmp/session.wav", {
        promotion: {
          scope: "current_capture",
          audioOffsetMs: 60_000,
          replaceTranscriptId: "transcript-current-live",
          startedAt: 123_000,
        },
      });
    });

    expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        promotion: {
          scope: "current_capture",
          audio_offset_ms: 60_000,
          replace_transcript_id: "transcript-current-live",
          started_at: 123_000,
        },
        words: [expect.objectContaining({ text: "answer", channel: 1 })],
        hints: [
          expect.objectContaining({
            type: "provider_speaker_index",
            value: '{"provider":"deepgram","channel":1,"speaker_index":3}',
          }),
        ],
      }),
    );
  });

  test.each(["current_capture", "whole_session"] as const)(
    "keeps saved data when Rust reports truncation for %s refinement",
    async (scope) => {
      saveBatchTranscriptMock.mockResolvedValueOnce({
        status: "ok",
        data: { status: "truncated" },
      });
      startTranscriptionMock.mockImplementation(async (_params, options) => {
        options.handlePersist(
          [
            {
              text: "replacement",
              start_ms: 60_100,
              end_ms: 61_000,
              channel: 0,
            },
          ],
          [],
          { mode: "replace" },
        );
      });

      const { result } = renderHook(() => useRunBatch("session-1"));
      let error: unknown;
      await act(async () => {
        try {
          await result.current("/tmp/session.wav", {
            promotion:
              scope === "current_capture"
                ? {
                    scope,
                    audioOffsetMs: 60_000,
                    replaceTranscriptId: "transcript-current-live",
                    startedAt: 123_000,
                  }
                : { scope },
          });
        } catch (caught) {
          error = caught;
        }
      });

      expect(error).toMatchObject({
        message:
          "The new transcription returned much less text. Your saved transcript and recording were kept. Try transcribing again.",
      });
      expect(isTerminalTranscriptionError(error)).toBe(true);
      expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
        expect.objectContaining({
          promotion:
            scope === "current_capture"
              ? {
                  scope,
                  audio_offset_ms: 60_000,
                  replace_transcript_id: "transcript-current-live",
                  started_at: 123_000,
                }
              : { scope },
        }),
      );
      expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
      expect(notifyBatchCompletedMock).not.toHaveBeenCalled();
    },
  );

  test.each(["current_capture", "whole_session"] as const)(
    "keeps existing data when Rust cannot save the %s transcript",
    async (scope) => {
      saveBatchTranscriptMock.mockResolvedValueOnce({
        status: "error",
        error: "database is unavailable",
      });
      startTranscriptionMock.mockImplementation(async (_params, options) => {
        options.handlePersist(
          [{ text: "replacement", start_ms: 0, end_ms: 100, channel: 0 }],
          [],
        );
      });
      const { result } = renderHook(() => useRunBatch("session-1"));

      await expect(
        act(async () => {
          await result.current("/tmp/session.wav", {
            promotion:
              scope === "current_capture"
                ? {
                    scope,
                    audioOffsetMs: 0,
                    replaceTranscriptId: "live-current",
                    startedAt: 123_000,
                  }
                : { scope },
          });
        }),
      ).rejects.toBeInstanceOf(BatchResponseProcessingError);

      expect(saveBatchTranscriptMock).toHaveBeenCalledOnce();
      expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
      expect(notifyBatchCompletedMock).not.toHaveBeenCalled();
    },
  );

  test("retains recovery audio when the batch has no current-capture words", async () => {
    saveBatchTranscriptMock.mockResolvedValueOnce({
      status: "ok",
      data: { status: "empty_current_capture" },
    });
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "old", start_ms: 10_000, end_ms: 10_500, channel: 0 }],
        [],
        { mode: "replace" },
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => {
        await result.current("/tmp/session.wav", {
          promotion: {
            scope: "current_capture",
            audioOffsetMs: 60_000,
            replaceTranscriptId: "transcript-current-live",
            startedAt: 123_000,
          },
        });
      }),
    ).rejects.toThrow(EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE);

    expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        promotion: {
          scope: "current_capture",
          audio_offset_ms: 60_000,
          replace_transcript_id: "transcript-current-live",
          started_at: 123_000,
        },
        words: [expect.objectContaining({ text: "old", start_ms: 10_000 })],
      }),
    );
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("retains recovery audio when the batch emits no words", async () => {
    saveBatchTranscriptMock.mockResolvedValueOnce({
      status: "ok",
      data: { status: "empty_current_capture" },
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => {
        await result.current("/tmp/session.wav", {
          promotion: {
            scope: "current_capture",
            audioOffsetMs: 60_000,
            replaceTranscriptId: "transcript-current-live",
            startedAt: 123_000,
          },
        });
      }),
    ).rejects.toThrow(EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE);

    expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        promotion: {
          scope: "current_capture",
          audio_offset_ms: 60_000,
          replace_transcript_id: "transcript-current-live",
          started_at: 123_000,
        },
        words: [],
      }),
    );
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("does not finalize a batch when Rust saving returns an error result", async () => {
    saveBatchTranscriptMock.mockResolvedValueOnce({
      status: "error",
      error: "save IPC failed",
    });
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "replacement", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });
    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => await result.current("/tmp/session.wav")),
    ).rejects.toBeInstanceOf(BatchResponseProcessingError);

    expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        promotion: { scope: "preserve_existing" },
        mark_audio_complete: true,
      }),
    );
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("does not replace the live transcript when batch transcription fails", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "partial", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
      throw new Error("provider failed");
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => {
        await result.current("/tmp/session.wav");
      }),
    ).rejects.toThrow("provider failed");

    expect(saveBatchTranscriptMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("passes selected transcription languages to batch transcription", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
        model: "soniqo-parakeet-batch",
        baseUrl: "soniqo://local",
        apiKey: "",
      },
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "de" : ["en"],
    );
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "soniqo",
        model: "soniqo-parakeet-batch",
        languages: ["de", "en"],
      }),
      expect.any(Object),
    );
  });

  test("uses an explicit local batch target for speaker refinement", async () => {
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", {
        provider: "soniqo",
        model: "soniqo-parakeet-batch",
        baseUrl: "soniqo://local",
        apiKey: "",
        notifyOnCompletion: false,
      });
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "soniqo",
        model: "soniqo-parakeet-batch",
        base_url: "soniqo://local",
        api_key: "",
      }),
      expect.objectContaining({ notifyOnCompletion: false }),
    );
    expect(toastWarningMock).not.toHaveBeenCalled();
    expect(notifyBatchCompletedMock).not.toHaveBeenCalled();
  });

  test.each(["windows", "linux"] as const)(
    "reports a language mismatch instead of a platform gap when Mistral is configured on %s",
    async (currentPlatform) => {
      platformMock.mockReturnValue(currentPlatform);
      isSupportedLanguagesBatchMock.mockResolvedValue(false);
      useSTTConnectionMock.mockReturnValue({
        conn: {
          provider: "mistral",
          model: "voxtral-mini-2602",
          baseUrl: "https://api.mistral.ai/v1",
          apiKey: "mistral-key",
        },
      });

      const { result } = renderHook(() => useRunBatch("session-1"));

      await expect(
        act(async () => {
          await result.current("/tmp/session.wav");
        }),
      ).rejects.toThrow(
        "voxtral-mini-2602 is not available for batch transcription with the selected languages",
      );

      expect(startTranscriptionMock).not.toHaveBeenCalled();
      expect(toastWarningMock).not.toHaveBeenCalled();
    },
  );

  test.each(["windows", "linux", "macos"] as const)(
    "uses custom Deepgram-compatible batch endpoints on %s",
    async (currentPlatform) => {
      platformMock.mockReturnValue(currentPlatform);
      useSTTConnectionMock.mockReturnValue({
        conn: {
          provider: "custom",
          model: "realtime-only",
          baseUrl: "https://custom.test",
          apiKey: "custom-key",
        },
      });

      const { result } = renderHook(() => useRunBatch("session-1"));

      await act(async () => {
        await result.current("/tmp/session.wav");
      });

      expect(startTranscriptionMock).toHaveBeenCalledWith(
        expect.objectContaining({
          provider: "deepgram",
          model: "realtime-only",
          base_url: "https://custom.test",
          api_key: "custom-key",
        }),
        expect.any(Object),
      );
      expect(toastWarningMock).not.toHaveBeenCalled();
    },
  );

  test.each([
    { isPaid: false, name: "rejects Soniqo for unpaid users" },
    { isPaid: true, name: "falls back to cloud for paid users" },
  ])("never invokes Soniqo on Intel macOS: $name", async ({ isPaid }) => {
    archMock.mockReturnValue("x86_64");
    useBillingAccessMock.mockReturnValue({ isPaid });
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
        model: "soniqo-parakeet-batch",
        baseUrl: "soniqo://local",
        apiKey: "",
      },
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    if (!isPaid) {
      await expect(
        act(async () => {
          await result.current("/tmp/session.wav");
        }),
      ).rejects.toThrow(
        "soniqo-parakeet-batch is not available for batch transcription on this platform",
      );
      expect(startTranscriptionMock).not.toHaveBeenCalled();
      return;
    }

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "anarlog",
        model: "cloud",
        base_url: "https://api.test/stt",
        api_key: "paid-token",
      }),
      expect.any(Object),
    );
  });

  test("falls back to hosted cloud transcription for paid users", async () => {
    isSupportedLanguagesBatchMock.mockResolvedValue(false);
    useBillingAccessMock.mockReturnValue({
      isPaid: true,
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "anarlog",
        model: "cloud",
        base_url: "https://api.test/stt",
        api_key: "paid-token",
      }),
      expect.any(Object),
    );
    expect(toastWarningMock).toHaveBeenCalledWith(
      "Using a batch transcription provider",
      expect.objectContaining({
        description:
          "nova-3 is not available for batch transcription. Using Pro cloud transcription instead.",
      }),
    );
  });

  test("uses a request-ready cloud token before transcription starts", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
        model: "cloud",
        baseUrl: "https://api.test/stt",
        apiKey: "stale-token",
      },
    });
    useBillingAccessMock.mockReturnValue({ isPaid: true });
    getSessionForRequestMock.mockResolvedValue({
      access_token: "request-ready-token",
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledTimes(1);
    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({ api_key: "request-ready-token" }),
      expect.any(Object),
    );
  });

  test("falls back to the current cloud token when refresh is unavailable", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
        model: "cloud",
        baseUrl: "https://api.test/stt",
        apiKey: "stale-token",
      },
    });
    useBillingAccessMock.mockReturnValue({ isPaid: true });
    getSessionForRequestMock.mockRejectedValue(new Error("offline"));
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledTimes(1);
    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({ api_key: "paid-token" }),
      expect.any(Object),
    );
  });

  test("refreshes an expired cloud token and retries transcription once", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
        model: "cloud",
        baseUrl: "https://api.test/stt",
        apiKey: "stale-token",
      },
    });
    useAuthMock.mockReturnValue({
      session: {
        access_token: "stale-token",
        user: { id: "user-1" },
      },
      getSessionForRequest: getSessionForRequestMock,
      refreshSession: refreshSessionMock,
    });
    getSessionForRequestMock.mockResolvedValue({
      access_token: "stale-token",
    });
    refreshSessionMock.mockResolvedValue({ access_token: "fresh-token" });
    startTranscriptionMock
      .mockImplementationOnce(async (_params, options) => {
        options.handlePersist(
          [{ text: "stale", start_ms: 0, end_ms: 100, channel: 0 }],
          [],
        );
        throw new Error(
          "Authentication failed. Please check your API key in settings.",
        );
      })
      .mockImplementationOnce(async (_params, options) => {
        options.handlePersist(
          [{ text: "fresh", start_ms: 0, end_ms: 100, channel: 0 }],
          [],
        );
      });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(refreshSessionMock).toHaveBeenCalledTimes(1);
    expect(startTranscriptionMock).toHaveBeenCalledTimes(2);
    expect(startTranscriptionMock).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ api_key: "fresh-token" }),
      expect.any(Object),
    );
    expect(saveBatchTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        words: [expect.objectContaining({ text: "fresh" })],
      }),
    );
  });
});

describe("getSessionSpeakerCount", () => {
  test("counts distinct session participants plus the current user", () => {
    expect(
      getSessionSpeakerCount(["human-a", "human-a", "human-b"], "self"),
    ).toBe(3);
  });

  test("returns undefined until at least two speakers are known", () => {
    expect(getSessionSpeakerCount(["human-a"], null)).toBe(undefined);
  });
});
