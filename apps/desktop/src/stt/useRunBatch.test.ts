import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { beginCloudsyncActivity, endCloudsyncActivity } from "@anlg/plugin-db";

import {
  canRunBatchTranscription,
  EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR_MESSAGE,
  getBatchFallbackTarget,
  getBatchProvider,
  getSessionSpeakerCount,
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
    isAnarlogLocalSttModel: (
      provider: string | null | undefined,
      model: string | null | undefined,
    ) =>
      provider === "anarlog" &&
      typeof model === "string" &&
      (model.startsWith("soniqo-") ||
        model.startsWith("am-") ||
        model.startsWith("Quantized")),
    isSupportedLanguagesBatch: isSupportedLanguagesBatchMock,
  };
});

vi.mock("~/stt/queries", () => ({
  createTranscript: createTranscriptMock,
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

  test("maps Cloudflare Workers AI to the Deepgram-compatible batch provider", () => {
    expect(getBatchProvider("cloudflare_workers_ai", "nova-3")).toBe(
      "deepgram",
    );
  });

  test("maps local soniqo models to soniqo batch provider", () => {
    expect(getBatchProvider("anarlog", "soniqo-parakeet-batch")).toBe("soniqo");
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

describe("useRunBatch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    archMock.mockReturnValue("aarch64");
    platformMock.mockReturnValue("macos");

    let nextId = 0;
    idMock.mockImplementation(() => `generated-${++nextId}`);
    createTranscriptMock.mockResolvedValue(undefined);
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

  test("appends only the current capture when prior transcript audio is partial", async () => {
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
