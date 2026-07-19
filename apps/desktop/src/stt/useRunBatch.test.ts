import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { getSessionKeywords } from "./useKeywords";
import {
  canRunBatchTranscription,
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
  useGoogleCalendarDataStateMock,
  hasGoogleCalendarDataMock,
  useAuthMock,
  useBillingAccessMock,
  useConfigValueMock,
  isSupportedLanguagesBatchMock,
  sonnerToastWarningMock,
  deleteProcessedAudioForRetentionMock,
  createTranscriptMock,
  appendTranscriptWordsAndHintsMock,
  idMock,
} = vi.hoisted(() => ({
  startTranscriptionMock: vi.fn(),
  useListenerMock: vi.fn(),
  useSessionMock: vi.fn(),
  useSessionParticipantsMock: vi.fn(),
  useSTTConnectionMock: vi.fn(),
  useGoogleCalendarDataStateMock: vi.fn(),
  hasGoogleCalendarDataMock: vi.fn(),
  useAuthMock: vi.fn(),
  useBillingAccessMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  isSupportedLanguagesBatchMock: vi.fn(),
  sonnerToastWarningMock: vi.fn(),
  deleteProcessedAudioForRetentionMock: vi.fn(),
  createTranscriptMock: vi.fn(),
  appendTranscriptWordsAndHintsMock: vi.fn(),
  idMock: vi.fn(),
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

vi.mock("~/ai/hooks/useGoogleCalendarDataState", () => ({
  useGoogleCalendarDataState: useGoogleCalendarDataStateMock,
}));

vi.mock("~/ai/google-calendar-data", () => ({
  hasGoogleCalendarData: hasGoogleCalendarDataMock,
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
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
    isSupportedLanguagesBatch: isSupportedLanguagesBatchMock,
  };
});

vi.mock("~/stt/queries", () => ({
  appendTranscriptWordsAndHints: appendTranscriptWordsAndHintsMock,
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
    expect(getBatchProvider("hyprnote", "soniqo-parakeet-batch")).toBe(
      "soniqo",
    );
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
      }),
    ).toEqual({
      provider: "hyprnote",
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
      }),
    ).toEqual({
      provider: "soniqo",
      model: "soniqo-parakeet-batch",
      baseUrl: "soniqo://local",
      apiKey: "",
      label: "Soniqo batch transcription",
    });
  });
});

describe("useRunBatch", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    let nextId = 0;
    idMock.mockImplementation(() => `generated-${++nextId}`);
    createTranscriptMock.mockResolvedValue(undefined);
    appendTranscriptWordsAndHintsMock.mockResolvedValue(undefined);
    deleteProcessedAudioForRetentionMock.mockResolvedValue(undefined);
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
    useGoogleCalendarDataStateMock.mockReturnValue("absent");
    hasGoogleCalendarDataMock.mockResolvedValue(false);
    useAuthMock.mockReturnValue({
      session: {
        access_token: "paid-token",
        user: { id: "user-1" },
      },
    });
    useBillingAccessMock.mockReturnValue({
      isPaid: false,
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "en" : [],
    );
  });

  test("waits for streamed SQLite persists before retention", async () => {
    let resolveAppend: (() => void) | undefined;
    appendTranscriptWordsAndHintsMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveAppend = resolve;
        }),
    );
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "hello", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
      options.handlePersist(
        [{ text: "world", start_ms: 100, end_ms: 200, channel: 0 }],
        [],
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));
    const run = result.current("/tmp/session.wav");

    await waitFor(() => {
      expect(appendTranscriptWordsAndHintsMock).toHaveBeenCalledTimes(1);
    });
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();

    resolveAppend?.();
    await act(async () => await run);

    expect(createTranscriptMock).toHaveBeenCalledTimes(1);
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledTimes(1);
    expect(
      appendTranscriptWordsAndHintsMock.mock.invocationCallOrder[0],
    ).toBeLessThan(
      deleteProcessedAudioForRetentionMock.mock.invocationCallOrder[0],
    );
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
    expect(appendTranscriptWordsAndHintsMock).not.toHaveBeenCalled();
  });

  test("flushes default batch persists before rethrowing transcription errors", async () => {
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

    expect(createTranscriptMock).toHaveBeenCalledTimes(1);
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("passes selected transcription languages to batch transcription", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
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

  test("withholds Google attendee speaker hints from off-device batch STT", async () => {
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "Existing memo",
    });
    useGoogleCalendarDataStateMock.mockReturnValue("present");
    useSessionParticipantsMock.mockReturnValue([
      {
        source: "auto",
        humanId: "google-attendee-human",
      },
    ]);
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", {
        numSpeakers: 4,
        minSpeakers: 2,
        maxSpeakers: 6,
      });
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        num_speakers: undefined,
        min_speakers: undefined,
        max_speakers: undefined,
      }),
      expect.any(Object),
    );
    expect(getSessionKeywords).toHaveBeenCalledWith({
      sessionId: "session-1",
      dictionaryTerms: [],
      allowCalendarDerivedHints: false,
    });
  });

  test("rechecks Google data before starting off-device batch STT", async () => {
    useSessionParticipantsMock.mockReturnValue([
      { source: "auto", humanId: "google-attendee-human" },
    ]);
    hasGoogleCalendarDataMock.mockResolvedValue(true);
    vi.mocked(getSessionKeywords).mockImplementation(
      async ({ allowCalendarDerivedHints }) =>
        allowCalendarDerivedHints ? ["Google event"] : ["safe note"],
    );
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", { numSpeakers: 4 });
    });

    expect(getSessionKeywords).toHaveBeenNthCalledWith(1, {
      sessionId: "session-1",
      dictionaryTerms: [],
      allowCalendarDerivedHints: true,
    });
    expect(getSessionKeywords).toHaveBeenNthCalledWith(2, {
      sessionId: "session-1",
      dictionaryTerms: [],
      allowCalendarDerivedHints: false,
    });
    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        keywords: ["safe note"],
        num_speakers: undefined,
      }),
      expect.any(Object),
    );
  });

  test("redacts batch hints when the request-time check fails", async () => {
    hasGoogleCalendarDataMock.mockRejectedValue(new Error("database offline"));
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", {
        minSpeakers: 2,
        maxSpeakers: 5,
      });
    });

    expect(getSessionKeywords).toHaveBeenLastCalledWith({
      sessionId: "session-1",
      dictionaryTerms: [],
      allowCalendarDerivedHints: false,
    });
    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        min_speakers: undefined,
        max_speakers: undefined,
      }),
      expect.any(Object),
    );
  });

  test("retains Google speaker hints for on-device batch STT", async () => {
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "Existing memo",
    });
    useGoogleCalendarDataStateMock.mockReturnValue("present");
    useSessionParticipantsMock.mockReturnValue([
      { source: "auto", humanId: "google-attendee-human" },
    ]);
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
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
      expect.objectContaining({ num_speakers: 2 }),
      expect.any(Object),
    );
    expect(getSessionKeywords).toHaveBeenCalledWith({
      sessionId: "session-1",
      dictionaryTerms: [],
      allowCalendarDerivedHints: true,
    });
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
        provider: "hyprnote",
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
