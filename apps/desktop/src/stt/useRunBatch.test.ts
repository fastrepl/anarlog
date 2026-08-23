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
  reconcileRefinedSpeakerClusters,
} from "./useRunBatch";
import { useRunBatch } from "./useRunBatch";

const {
  startTranscriptionMock,
  useListenerMock,
  useSessionMock,
  useSessionParticipantsMock,
  useSTTConnectionMock,
  useAuthMock,
  refreshSessionMock,
  useBillingAccessMock,
  useConfigValueMock,
  isSupportedLanguagesBatchMock,
  sonnerToastWarningMock,
  deleteProcessedAudioForRetentionMock,
  markSessionAudioTranscriptionCompleteMock,
  createTranscriptMock,
  getTranscriptRecordMock,
  idMock,
  archMock,
  platformMock,
} = vi.hoisted(() => ({
  startTranscriptionMock: vi.fn(),
  useListenerMock: vi.fn(),
  useSessionMock: vi.fn(),
  useSessionParticipantsMock: vi.fn(),
  useSTTConnectionMock: vi.fn(),
  useAuthMock: vi.fn(),
  refreshSessionMock: vi.fn(),
  useBillingAccessMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  isSupportedLanguagesBatchMock: vi.fn(),
  sonnerToastWarningMock: vi.fn(),
  deleteProcessedAudioForRetentionMock: vi.fn(),
  markSessionAudioTranscriptionCompleteMock: vi.fn(),
  createTranscriptMock: vi.fn(),
  getTranscriptRecordMock: vi.fn(),
  idMock: vi.fn(),
  archMock: vi.fn(),
  platformMock: vi.fn(),
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
  sonnerToast: {
    warning: sonnerToastWarningMock,
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

vi.mock("~/session/attachments", () => ({
  markSessionAudioTranscriptionComplete:
    markSessionAudioTranscriptionCompleteMock,
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

vi.mock("~/stt/queries", () => ({
  createTranscript: createTranscriptMock,
  getTranscriptRecord: getTranscriptRecordMock,
}));

describe("getBatchProvider", () => {
  test("maps pyannote to the batch transcription provider", () => {
    expect(getBatchProvider("pyannote", "parakeet-tdt-0.6b-v3")).toBe(
      "pyannote",
    );
  });

  test("keeps openai mapped to the batch transcription provider", () => {
    expect(getBatchProvider("openai", "gpt-4o-transcribe")).toBe("openai");
  });

  test("keeps cartesia mapped to the batch transcription provider", () => {
    expect(getBatchProvider("cartesia", "ink-2")).toBe("cartesia");
  });

  test("maps Cohere to the batch transcription provider", () => {
    expect(getBatchProvider("cohere", "cohere-transcribe-03-2026")).toBe(
      "cohere",
    );
  });

  test("maps Mistral to the batch transcription provider", () => {
    expect(getBatchProvider("mistral", "voxtral-mini-2602")).toBe("mistral");
  });

  test.each([
    ["aws_transcribe", "amazon-transcribe"],
    ["azure_speech", "fast-transcription"],
    ["google_cloud", "latest_long"],
    ["groq", "whisper-large-v3-turbo"],
    ["openrouter", "openai/gpt-4o-mini-transcribe"],
    ["siliconflow", "FunAudioLLM/SenseVoiceSmall"],
    ["zai", "glm-asr-2512"],
    ["revai", "machine"],
    ["speechmatics", "enhanced"],
    ["together", "openai/whisper-large-v3"],
    ["xai", "xai-stt"],
  ] as const)("maps %s to its direct batch provider", (provider, model) => {
    expect(getBatchProvider(provider, model)).toBe(provider);
  });

  test("maps Cloudflare Workers AI to the Deepgram-compatible batch provider", () => {
    expect(getBatchProvider("cloudflare_workers_ai", "nova-3")).toBe(
      "deepgram",
    );
  });

  test("maps local soniqo models to soniqo batch provider", () => {
    expect(getBatchProvider("anarlog", "soniqo-parakeet-batch")).toBe("soniqo");
    expect(getBatchProvider("soniqo", "soniqo-parakeet-batch")).toBe("soniqo");
  });

  test("maps Apple Speech to its batch runtime provider", () => {
    expect(getBatchProvider("apple_speech", "apple-speech")).toBe(
      "applespeech",
    );
  });

  test("maps local model files to whisper.cpp", () => {
    expect(getBatchProvider("local_file", "local-file")).toBe("whispercpp");
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

  test.each(["windows", "linux"] as const)(
    "does not use local Soniqo on %s",
    (currentPlatform) => {
      expect(
        getBatchFallbackTarget({
          isPaid: false,
          accessToken: null,
          apiBaseUrl: "https://api.test",
          currentPlatform,
          currentArch: "x86_64",
        }),
      ).toBeNull();
    },
  );

  test("does not use local Soniqo on Intel macOS", () => {
    expect(
      getBatchFallbackTarget({
        isPaid: false,
        accessToken: null,
        apiBaseUrl: "https://api.test",
        currentPlatform: "macos",
        currentArch: "x86_64",
      }),
    ).toBeNull();
  });
});

describe("reconcileRefinedSpeakerClusters", () => {
  test("collapses split batch clusters onto dominant live clusters", () => {
    const source = {
      id: "live-transcript",
      ownerUserId: "user-1",
      sessionId: "session-1",
      startedAt: 0,
      words: [
        {
          id: "live-lex-1",
          text: "question",
          start_ms: 0,
          end_ms: 100,
          channel: 1,
        },
        {
          id: "live-george-1",
          text: "answer",
          start_ms: 100,
          end_ms: 200,
          channel: 1,
        },
        {
          id: "live-lex-2",
          text: "follow up",
          start_ms: 200,
          end_ms: 300,
          channel: 1,
        },
        {
          id: "live-george-2",
          text: "response",
          start_ms: 300,
          end_ms: 400,
          channel: 1,
        },
      ],
      speakerHints: [
        {
          id: "live-lex-1-provider",
          word_id: "live-lex-1",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 0 }),
        },
        {
          id: "live-george-1-provider",
          word_id: "live-george-1",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 1 }),
        },
        {
          id: "live-lex-2-provider",
          word_id: "live-lex-2",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 0 }),
        },
        {
          id: "live-george-2-provider",
          word_id: "live-george-2",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 1 }),
        },
      ],
    } satisfies Parameters<typeof reconcileRefinedSpeakerClusters>[0];
    const words = [
      {
        id: "batch-lex-primary",
        text: "question",
        start_ms: 0,
        end_ms: 100,
        channel: 1,
      },
      {
        id: "batch-george-primary",
        text: "answer",
        start_ms: 100,
        end_ms: 200,
        channel: 1,
      },
      {
        id: "batch-lex-split",
        text: "follow up",
        start_ms: 200,
        end_ms: 300,
        channel: 1,
      },
      {
        id: "batch-george-split",
        text: "response",
        start_ms: 300,
        end_ms: 400,
        channel: 1,
      },
    ];
    const hints = words.map((word, index) => ({
      id: `${word.id}-provider`,
      word_id: word.id,
      type: "provider_speaker_index" as const,
      value: JSON.stringify({
        provider: "anarlog",
        channel: 1,
        speaker_index: index,
      }),
    }));

    const result = reconcileRefinedSpeakerClusters(source, words, hints);

    expect(result.map((hint) => JSON.parse(hint.value).speaker_index)).toEqual([
      0, 1, 0, 1,
    ]);
  });

  test("keeps an ambiguous batch cluster unchanged", () => {
    const source = {
      id: "live-transcript",
      ownerUserId: "user-1",
      sessionId: "session-1",
      startedAt: 0,
      words: [
        {
          id: "live-a",
          text: "one",
          start_ms: 0,
          end_ms: 100,
          channel: 1,
        },
        {
          id: "live-b",
          text: "two",
          start_ms: 100,
          end_ms: 200,
          channel: 1,
        },
      ],
      speakerHints: [
        {
          id: "live-a-provider",
          word_id: "live-a",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 0 }),
        },
        {
          id: "live-b-provider",
          word_id: "live-b",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 1 }),
        },
      ],
    } satisfies Parameters<typeof reconcileRefinedSpeakerClusters>[0];
    const words = [
      {
        id: "batch-ambiguous",
        text: "one two",
        start_ms: 0,
        end_ms: 200,
        channel: 1,
      },
    ];
    const hints = [
      {
        id: "batch-ambiguous-provider",
        word_id: "batch-ambiguous",
        type: "provider_speaker_index" as const,
        value: JSON.stringify({ channel: 1, speaker_index: 4 }),
      },
    ];

    const result = reconcileRefinedSpeakerClusters(source, words, hints);

    expect(JSON.parse(result[0].value).speaker_index).toBe(4);
  });

  test("moves an unmapped batch cluster that collides with a live cluster", () => {
    const source = {
      id: "live-transcript",
      ownerUserId: "user-1",
      sessionId: "session-1",
      startedAt: 0,
      words: [
        {
          id: "live-speaker",
          text: "mapped",
          start_ms: 0,
          end_ms: 100,
          channel: 1,
        },
      ],
      speakerHints: [
        {
          id: "live-speaker-provider",
          word_id: "live-speaker",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 1 }),
        },
      ],
    } satisfies Parameters<typeof reconcileRefinedSpeakerClusters>[0];
    const words = [
      {
        id: "batch-mapped",
        text: "mapped",
        start_ms: 0,
        end_ms: 100,
        channel: 1,
      },
      {
        id: "batch-unmapped",
        text: "unmapped",
        start_ms: 200,
        end_ms: 300,
        channel: 1,
      },
    ];
    const hints = [
      {
        id: "batch-mapped-provider",
        word_id: "batch-mapped",
        type: "provider_speaker_index" as const,
        value: JSON.stringify({ channel: 1, speaker_index: 0 }),
      },
      {
        id: "batch-unmapped-provider",
        word_id: "batch-unmapped",
        type: "provider_speaker_index" as const,
        value: JSON.stringify({ channel: 1, speaker_index: 1 }),
      },
    ];

    const result = reconcileRefinedSpeakerClusters(source, words, hints);

    expect(result.map((hint) => JSON.parse(hint.value).speaker_index)).toEqual([
      1, 2,
    ]);
  });
});

describe("useRunBatch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    archMock.mockReturnValue("aarch64");
    platformMock.mockReturnValue("macos");

    let nextId = 0;
    idMock.mockImplementation(() => `generated-${++nextId}`);
    createTranscriptMock.mockResolvedValue(undefined);
    getTranscriptRecordMock.mockResolvedValue(null);
    deleteProcessedAudioForRetentionMock.mockResolvedValue(undefined);
    markSessionAudioTranscriptionCompleteMock.mockResolvedValue(undefined);
    isSupportedLanguagesBatchMock.mockResolvedValue(true);
    useListenerMock.mockImplementation((selector) =>
      selector({ startTranscription: startTranscriptionMock }),
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
      refreshSession: refreshSessionMock,
    });
    refreshSessionMock.mockResolvedValue(null);
    useBillingAccessMock.mockReturnValue({
      isPaid: false,
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "en" : [],
    );
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

    expect(createTranscriptMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();

    finishTranscription?.();
    await act(async () => await run);

    expect(beginCloudsyncActivity).toHaveBeenCalledWith(
      "transcription",
      "session-1:generated-1",
    );
    expect(createTranscriptMock).toHaveBeenCalledTimes(1);
    expect(createTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        replaceSession: true,
        words: [
          expect.objectContaining({ text: "hello" }),
          expect.objectContaining({ text: "world" }),
        ],
      }),
    );
    expect(markSessionAudioTranscriptionCompleteMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledTimes(1);
    expect(
      markSessionAudioTranscriptionCompleteMock.mock.invocationCallOrder[0],
    ).toBeLessThan(
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

    expect(createTranscriptMock).toHaveBeenCalledOnce();
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("does not make completed provider work retryable when persistence fails", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "recovered", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });
    createTranscriptMock.mockRejectedValueOnce(new Error("disk write failed"));

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
    expect(createTranscriptMock).not.toHaveBeenCalled();
  });

  test("promotes post-stop batch by replacing the live current capture", async () => {
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

    expect(createTranscriptMock).toHaveBeenCalledOnce();
    expect(createTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        replaceSession: false,
        replaceTranscriptId: "transcript-current-live",
        startedAt: 123_000,
        words: [
          expect.objectContaining({
            text: "new",
            start_ms: 100,
            end_ms: 500,
          }),
        ],
        speakerHints: [
          expect.objectContaining({
            value: expect.stringContaining('"speaker_index":1'),
          }),
        ],
      }),
    );
  });

  test("does not carry legacy inferred identities into a refined capture", async () => {
    getTranscriptRecordMock.mockResolvedValue({
      id: "transcript-current-live",
      ownerUserId: "user-1",
      sessionId: "session-1",
      startedAt: 123_000,
      words: [
        {
          id: "live-word",
          text: "answer",
          start_ms: 100,
          end_ms: 500,
          channel: 1,
        },
      ],
      speakerHints: [
        {
          id: "live-provider",
          word_id: "live-word",
          type: "provider_speaker_index",
          value: JSON.stringify({ channel: 1, speaker_index: 0 }),
        },
        {
          id: "live-inferred-identity",
          word_id: "live-word",
          type: "automatic_speaker_assignment",
          value: JSON.stringify({
            human_id: "human-1",
            confidence: 0.93,
            source: "enhance",
          }),
        },
      ],
    });
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

    const speakerHints = createTranscriptMock.mock.calls[0]?.[0].speakerHints;
    expect(speakerHints).toHaveLength(1);
    expect(speakerHints[0]).toMatchObject({
      type: "provider_speaker_index",
    });
  });

  test("retains recovery audio when the batch has no current-capture words", async () => {
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

    expect(createTranscriptMock).not.toHaveBeenCalled();
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("retains recovery audio when the batch emits no words", async () => {
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

    expect(createTranscriptMock).not.toHaveBeenCalled();
    expect(markSessionAudioTranscriptionCompleteMock).not.toHaveBeenCalled();
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

    expect(createTranscriptMock).not.toHaveBeenCalled();
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
    expect(sonnerToastWarningMock).not.toHaveBeenCalled();
  });

  test("falls back to local Soniqo when the selected provider is not batch-capable", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "custom",
        model: "realtime-only",
        baseUrl: "https://custom.test",
        apiKey: "custom-key",
      },
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "soniqo",
        model: "soniqo-parakeet-batch",
        base_url: "soniqo://local",
        api_key: "",
      }),
      expect.any(Object),
    );
    expect(sonnerToastWarningMock).toHaveBeenCalledWith(
      "Using a batch transcription provider",
      expect.objectContaining({
        description:
          "realtime-only is not available for batch transcription. Using Soniqo batch transcription instead.",
      }),
    );
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
      expect(sonnerToastWarningMock).not.toHaveBeenCalled();
    },
  );

  test.each(["windows", "linux"] as const)(
    "does not invoke Soniqo as a batch fallback on %s",
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

      await expect(
        act(async () => {
          await result.current("/tmp/session.wav");
        }),
      ).rejects.toThrow(
        "realtime-only is not available for batch transcription on this platform",
      );

      expect(startTranscriptionMock).not.toHaveBeenCalled();
      expect(sonnerToastWarningMock).not.toHaveBeenCalled();
    },
  );

  test("does not invoke Soniqo as a selected target or fallback on Intel macOS", async () => {
    archMock.mockReturnValue("x86_64");
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "anarlog",
        model: "soniqo-parakeet-batch",
        baseUrl: "soniqo://local",
        apiKey: "",
      },
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => {
        await result.current("/tmp/session.wav");
      }),
    ).rejects.toThrow(
      "soniqo-parakeet-batch is not available for batch transcription on this platform",
    );

    expect(startTranscriptionMock).not.toHaveBeenCalled();
  });

  test("falls back from local Soniqo to cloud for paid Intel Mac users", async () => {
    archMock.mockReturnValue("x86_64");
    useBillingAccessMock.mockReturnValue({ isPaid: true });
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
    expect(sonnerToastWarningMock).toHaveBeenCalledWith(
      "Using a batch transcription provider",
      expect.objectContaining({
        description:
          "nova-3 is not available for batch transcription. Using Pro cloud transcription instead.",
      }),
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
      refreshSession: refreshSessionMock,
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
    expect(createTranscriptMock).toHaveBeenCalledWith(
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
