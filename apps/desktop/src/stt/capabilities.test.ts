import { beforeEach, describe, expect, test, vi } from "vitest";

const { isSupportedLanguagesBatchMock, isSupportedLanguagesLiveMock } =
  vi.hoisted(() => ({
    isSupportedLanguagesBatchMock: vi.fn(),
    isSupportedLanguagesLiveMock: vi.fn(),
  }));

vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    isSupportedLanguagesBatch: isSupportedLanguagesBatchMock,
    isSupportedLanguagesLive: isSupportedLanguagesLiveMock,
  },
}));

import {
  getLiveTranscriptionConfig,
  getOnDeviceTranscriptionConfig,
  getOnDeviceTranscriptionMode,
  getSttModelTranscriptionMode,
  getTranscriptionLanguages,
  getUnsupportedDesktopLocalSttRepair,
  isConfiguredSttModel,
  isDesktopLocalSttAvailable,
  isLocalFileSttModel,
  isOnDeviceSttModel,
  isSupportedLanguagesBatch,
  isSupportedLanguagesLive,
  isRealtimeLocalModel,
  isSupportedLocalSttModel,
} from "./capabilities";

beforeEach(() => {
  vi.clearAllMocks();
  isSupportedLanguagesLiveMock.mockResolvedValue({
    status: "ok",
    data: true,
  });
  isSupportedLanguagesBatchMock.mockResolvedValue({
    status: "ok",
    data: true,
  });
});

describe("getOnDeviceTranscriptionMode", () => {
  test("uses live mode for realtime local models", () => {
    expect(getOnDeviceTranscriptionMode("soniqo-parakeet-streaming")).toBe(
      "live",
    );
  });

  test("uses batch mode for non-realtime local models", () => {
    expect(getOnDeviceTranscriptionMode("soniqo-qwen3-small")).toBe("batch");
  });

  test("keeps live mode when realtime local model has no Soniqo-supported language", () => {
    expect(
      getOnDeviceTranscriptionMode("soniqo-parakeet-streaming", ["ko"]),
    ).toBe("live");
  });

  test("keeps European Soniqo streaming languages live", () => {
    expect(
      getOnDeviceTranscriptionMode("soniqo-parakeet-streaming", ["de"]),
    ).toBe("live");
  });
});

describe("getSttModelTranscriptionMode", () => {
  test("distinguishes external batch and realtime model variants", () => {
    expect(getSttModelTranscriptionMode("local_file", "local-file")).toBe(
      "batch",
    );
    expect(getSttModelTranscriptionMode("openai", "gpt-live-transcribe")).toBe(
      "live",
    );
    expect(getSttModelTranscriptionMode("openai", "gpt-transcribe")).toBe(
      "batch",
    );
    expect(
      getSttModelTranscriptionMode("openai", "gpt-4o-transcribe-diarize"),
    ).toBe("batch");
    expect(getSttModelTranscriptionMode("elevenlabs", "scribe_v2")).toBe(
      "batch",
    );
    expect(
      getSttModelTranscriptionMode("elevenlabs", "scribe_v2_realtime"),
    ).toBe("live");
    expect(getSttModelTranscriptionMode("assemblyai", "universal-3-pro")).toBe(
      "batch",
    );
    expect(
      getSttModelTranscriptionMode("assemblyai", "universal-3-5-pro"),
    ).toBe("batch");
    expect(
      getSttModelTranscriptionMode("assemblyai", "universal-3-5-pro-realtime"),
    ).toBe("live");
    expect(getSttModelTranscriptionMode("assemblyai", "u3-rt-pro")).toBe(
      "live",
    );
    expect(getSttModelTranscriptionMode("mistral", "voxtral-mini-2602")).toBe(
      "batch",
    );
    expect(getSttModelTranscriptionMode("deepgram", "flux-general-multi")).toBe(
      "live",
    );
    expect(getSttModelTranscriptionMode("gladia", "solaria-3")).toBe("batch");
    expect(
      getSttModelTranscriptionMode(
        "google_generative_ai",
        "gemini-3.5-transcribe-live",
      ),
    ).toBe("live");
    expect(
      getSttModelTranscriptionMode(
        "google_generative_ai",
        "gemini-3.5-transcribe",
      ),
    ).toBe("batch");
    expect(
      getSttModelTranscriptionMode("cohere", "cohere-transcribe-03-2026"),
    ).toBe("batch");
    expect(getSttModelTranscriptionMode("smallestai", "pulse-pro")).toBe(
      "batch",
    );
    for (const [provider, model] of [
      ["groq", "whisper-large-v3-turbo"],
      ["openrouter", "openai/gpt-4o-mini-transcribe"],
      ["together", "openai/whisper-large-v3"],
      ["speechmatics", "enhanced"],
      ["azure_speech", "fast-transcription"],
      ["google_cloud", "latest_long"],
      ["aws_transcribe", "amazon-transcribe"],
      ["revai", "machine"],
      ["pyannote", "parakeet-tdt-0.6b-v3"],
      ["aquavoice", "avalon-v1.5"],
      ["cohere", "cohere-transcribe-arabic-07-2026"],
    ]) {
      expect(getSttModelTranscriptionMode(provider, model)).toBe("batch");
    }
  });

  test("leaves models without an explicit mode to provider inference", () => {
    expect(getSttModelTranscriptionMode("deepgram", "nova-3-general")).toBe(
      undefined,
    );
    expect(getSttModelTranscriptionMode("xai", "xai-stt")).toBeUndefined();
    expect(getSttModelTranscriptionMode("smallestai", "pulse")).toBeUndefined();
  });
});

describe("isSupportedLocalSttModel", () => {
  test("accepts shipped local STT model families", () => {
    expect(isSupportedLocalSttModel("soniqo-parakeet-streaming")).toBe(true);
    expect(isSupportedLocalSttModel("apple-speech")).toBe(true);
    expect(isSupportedLocalSttModel("am-parakeet-v3")).toBe(true);
    expect(isSupportedLocalSttModel("QuantizedSmallEn")).toBe(true);
  });

  test("rejects cloud, local LLM, and removed local model ids", () => {
    expect(isSupportedLocalSttModel("cloud")).toBe(false);
    expect(isSupportedLocalSttModel("Llama3p2_3bQ4")).toBe(false);
    expect(isSupportedLocalSttModel("removed-local-model")).toBe(false);
  });
});

describe("isOnDeviceSttModel", () => {
  test("matches models to their dedicated on-device provider", () => {
    expect(isOnDeviceSttModel("soniqo", "soniqo-parakeet-streaming")).toBe(
      true,
    );
    expect(isOnDeviceSttModel("apple_speech", "apple-speech")).toBe(true);
    expect(isOnDeviceSttModel("soniqo", "apple-speech")).toBe(false);
    expect(isOnDeviceSttModel("apple_speech", "soniqo-parakeet-batch")).toBe(
      false,
    );
  });

  test("keeps legacy Anarlog local selections working during migration", () => {
    expect(isOnDeviceSttModel("anarlog", "soniqo-parakeet-streaming")).toBe(
      true,
    );
  });
});

describe("isLocalFileSttModel", () => {
  test("matches only the stable local-file provider and model ids", () => {
    expect(isLocalFileSttModel("local_file", "local-file")).toBe(true);
    expect(isLocalFileSttModel("local_file", "ggml-small.bin")).toBe(false);
    expect(isLocalFileSttModel("anarlog", "local-file")).toBe(false);
  });
});

describe("getOnDeviceTranscriptionConfig", () => {
  test("keeps languages Apple Speech supports but Parakeet does not", () => {
    expect(getOnDeviceTranscriptionConfig("apple-speech", ["ko"])).toEqual({
      languages: ["ko"],
      transcriptionMode: "live",
    });
    expect(getOnDeviceTranscriptionConfig("apple-speech", ["ja"])).toEqual({
      languages: ["ja"],
      transcriptionMode: "live",
    });
  });

  test("still drops languages Apple Speech cannot transcribe", () => {
    expect(getOnDeviceTranscriptionConfig("apple-speech", ["hi"])).toEqual({
      languages: [],
      transcriptionMode: "live",
    });
  });

  test("leaves the Parakeet language set untouched", () => {
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["ko"]),
    ).toEqual({ languages: [], transcriptionMode: "live" });
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["de"]),
    ).toEqual({ languages: ["de"], transcriptionMode: "live" });
  });
});

describe("isRealtimeLocalModel", () => {
  test("accepts the streaming-capable local models", () => {
    expect(isRealtimeLocalModel("soniqo-parakeet-streaming")).toBe(true);
    expect(isRealtimeLocalModel("apple-speech")).toBe(true);
  });

  test("rejects batch-only local models", () => {
    expect(isRealtimeLocalModel("soniqo-parakeet-batch")).toBe(false);
    expect(isRealtimeLocalModel("am-parakeet-v3")).toBe(false);
  });
});

describe("isConfiguredSttModel", () => {
  test("requires known model ids for Anarlog STT", () => {
    expect(isConfiguredSttModel("anarlog", "cloud")).toBe(true);
    expect(isConfiguredSttModel("anarlog", "soniqo-qwen3-small")).toBe(true);
    expect(isConfiguredSttModel("anarlog", "removed-local-model")).toBe(false);
  });

  test("requires models from the selected built-in provider", () => {
    expect(isConfiguredSttModel("soniqo", "soniqo-parakeet-batch")).toBe(true);
    expect(isConfiguredSttModel("soniqo", "apple-speech")).toBe(false);
    expect(isConfiguredSttModel("apple_speech", "apple-speech")).toBe(true);
    expect(
      isConfiguredSttModel("apple_speech", "soniqo-parakeet-streaming"),
    ).toBe(false);
    expect(isConfiguredSttModel("local_file", "local-file")).toBe(true);
    expect(isConfiguredSttModel("local_file", "ggml-small.bin")).toBe(false);
  });

  test("allows custom model ids for external providers", () => {
    expect(isConfiguredSttModel("custom", "whisper-large-v3")).toBe(true);
  });
});

describe("getUnsupportedDesktopLocalSttRepair", () => {
  test("reports local STT only on Apple Silicon", () => {
    expect(isDesktopLocalSttAvailable("macos", "aarch64")).toBe(true);
    expect(isDesktopLocalSttAvailable("macos", "x86_64")).toBe(false);
    expect(isDesktopLocalSttAvailable("windows", "aarch64")).toBe(false);
  });

  test.each(["windows", "linux"])(
    "uses hosted transcription for entitled users on %s",
    (currentPlatform) => {
      expect(
        getUnsupportedDesktopLocalSttRepair(
          currentPlatform,
          "x86_64",
          "anarlog",
          "soniqo-parakeet-streaming",
          true,
        ),
      ).toEqual({ provider: "anarlog", model: "cloud" });
    },
  );

  test.each(["windows", "linux"])(
    "requires a new provider selection for free users on %s",
    (currentPlatform) => {
      expect(
        getUnsupportedDesktopLocalSttRepair(
          currentPlatform,
          "x86_64",
          "anarlog",
          "am-parakeet-v3",
          false,
        ),
      ).toEqual({ provider: "", model: "" });
    },
  );

  test("keeps supported Apple-local selections on Apple Silicon", () => {
    expect(
      getUnsupportedDesktopLocalSttRepair(
        "macos",
        "aarch64",
        "anarlog",
        "soniqo-parakeet-streaming",
        false,
      ),
    ).toBeNull();
  });

  test("repairs local model files on unsupported platforms", () => {
    expect(
      getUnsupportedDesktopLocalSttRepair(
        "linux",
        "x86_64",
        "local_file",
        "local-file",
        true,
      ),
    ).toEqual({ provider: "anarlog", model: "cloud" });
  });

  test.each([
    [true, { provider: "anarlog", model: "cloud" }],
    [false, { provider: "", model: "" }],
  ])(
    "repairs unsupported Intel Mac local transcription when cloud access is %s",
    (canUseCloud, expected) => {
      expect(
        getUnsupportedDesktopLocalSttRepair(
          "macos",
          "x86_64",
          "anarlog",
          "soniqo-parakeet-streaming",
          canUseCloud,
        ),
      ).toEqual(expected);
    },
  );

  test("does not rewrite cloud or BYOK selections", () => {
    expect(
      getUnsupportedDesktopLocalSttRepair(
        "windows",
        "x86_64",
        "anarlog",
        "cloud",
        true,
      ),
    ).toBeNull();
    expect(
      getUnsupportedDesktopLocalSttRepair(
        "linux",
        "x86_64",
        "deepgram",
        "nova-3-general",
        false,
      ),
    ).toBeNull();
  });
});

describe("getOnDeviceTranscriptionConfig", () => {
  test("uses the first supported language for realtime local models", () => {
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["en", "ko"]),
    ).toEqual({
      languages: ["en"],
      transcriptionMode: "live",
    });
  });

  test("keeps German live even when English is an additional language", () => {
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["de", "en"]),
    ).toEqual({
      languages: ["de"],
      transcriptionMode: "live",
    });
  });

  test("drops unsupported Soniqo language hints instead of forcing batch", () => {
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["ko"]),
    ).toEqual({
      languages: [],
      transcriptionMode: "live",
    });
  });
});

describe("getLiveTranscriptionConfig", () => {
  test("runs local model files after recording", async () => {
    await expect(
      getLiveTranscriptionConfig({
        provider: "local_file",
        model: "local-file",
        languages: ["en", "ko"],
      }),
    ).resolves.toEqual({
      languages: ["en", "ko"],
      transcriptionMode: "batch",
    });
    expect(isSupportedLanguagesLiveMock).not.toHaveBeenCalled();
  });

  test("uses the dedicated OpenAI live model during recording", async () => {
    await expect(
      getLiveTranscriptionConfig({
        provider: "openai",
        model: "gpt-live-transcribe",
        languages: ["en", "ko"],
      }),
    ).resolves.toEqual({
      languages: ["en", "ko"],
      transcriptionMode: "live",
    });
  });

  test("uses the dedicated OpenAI file model after recording", async () => {
    await expect(
      getLiveTranscriptionConfig({
        provider: "openai",
        model: "gpt-transcribe",
        languages: ["en", "ko"],
      }),
    ).resolves.toEqual({
      languages: ["en", "ko"],
      transcriptionMode: "batch",
    });
    expect(isSupportedLanguagesLiveMock).not.toHaveBeenCalled();
  });

  test("forces ElevenLabs Scribe V2 into after-recording batch mode", async () => {
    await expect(
      getLiveTranscriptionConfig({
        provider: "elevenlabs",
        model: "scribe_v2",
        languages: ["en"],
      }),
    ).resolves.toEqual({
      languages: ["en"],
      transcriptionMode: "batch",
    });
    expect(isSupportedLanguagesLiveMock).not.toHaveBeenCalled();
  });

  test("keeps ElevenLabs Scribe V2 Realtime in live mode", async () => {
    await expect(
      getLiveTranscriptionConfig({
        provider: "elevenlabs",
        model: "scribe_v2_realtime",
        languages: ["en"],
      }),
    ).resolves.toEqual({
      languages: ["en"],
      transcriptionMode: "live",
    });
  });

  test("keeps all languages when the selected provider supports them live", async () => {
    const config = await getLiveTranscriptionConfig({
      provider: "deepgram",
      model: "nova-3-general",
      languages: ["en", "es"],
    });

    expect(config).toEqual({
      languages: ["en", "es"],
      transcriptionMode: undefined,
    });
    expect(isSupportedLanguagesLiveMock).toHaveBeenCalledTimes(1);
  });

  test("falls back to the main language when additional languages are unsupported live", async () => {
    isSupportedLanguagesLiveMock.mockImplementation(
      (_provider, _model, languages) =>
        Promise.resolve({
          status: "ok",
          data: languages.length === 1 && languages[0] === "en",
        }),
    );

    await expect(
      getLiveTranscriptionConfig({
        provider: "deepgram",
        model: "nova-3-general",
        languages: ["en", "ko"],
      }),
    ).resolves.toEqual({
      languages: ["en"],
      omittedLanguages: ["ko"],
      transcriptionMode: undefined,
    });
  });

  test("checks custom providers as Deepgram-compatible for language fallback", async () => {
    isSupportedLanguagesLiveMock.mockImplementation(
      (_provider, _model, languages) =>
        Promise.resolve({
          status: "ok",
          data: languages.length === 1 && languages[0] === "en",
        }),
    );

    await getLiveTranscriptionConfig({
      provider: "custom",
      model: "nova-3-general",
      languages: ["en", "ko"],
    });

    expect(isSupportedLanguagesLiveMock.mock.calls[0]?.[0]).toBe("deepgram");
  });

  test("checks Cloudflare Workers AI as Deepgram-compatible for language fallback", async () => {
    await getLiveTranscriptionConfig({
      provider: "cloudflare_workers_ai",
      model: "nova-3",
      languages: ["en", "ko"],
    });

    expect(isSupportedLanguagesLiveMock.mock.calls[0]?.[0]).toBe("deepgram");
  });

  test("checks Cloudflare Workers AI as Deepgram-compatible for live language support", async () => {
    await isSupportedLanguagesLive("cloudflare_workers_ai", "nova-3", ["en"]);

    expect(isSupportedLanguagesLiveMock.mock.calls[0]).toEqual([
      "deepgram",
      "nova-3",
      ["en"],
    ]);
  });

  test("maps the Apple Speech product provider to its runtime provider", async () => {
    await isSupportedLanguagesLive("apple_speech", "apple-speech", ["ko"]);

    expect(isSupportedLanguagesLiveMock.mock.calls[0]).toEqual([
      "apple-speech",
      "apple-speech",
      ["ko"],
    ]);
  });

  test("checks Cloudflare Workers AI as Deepgram-compatible for batch language support", async () => {
    await isSupportedLanguagesBatch("cloudflare_workers_ai", "nova-3", ["en"]);

    expect(isSupportedLanguagesBatchMock.mock.calls[0]).toEqual([
      "deepgram",
      "nova-3",
      ["en"],
    ]);
  });
});

describe("getTranscriptionLanguages", () => {
  test("prefers the main language before additional spoken languages", () => {
    expect(getTranscriptionLanguages("en", ["ko"])).toEqual(["en", "ko"]);
  });

  test("deduplicates regional variants by base language", () => {
    expect(getTranscriptionLanguages("en-US", ["en", "ko"])).toEqual([
      "en-US",
      "ko",
    ]);
  });
});
