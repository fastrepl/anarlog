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
  test.each([
    {
      model: "soniqo-parakeet-streaming",
      languages: undefined,
      expected: "live",
    },
    { model: "soniqo-qwen3-small", languages: undefined, expected: "batch" },
    {
      model: "soniqo-parakeet-streaming",
      languages: ["ko"],
      expected: "live",
    },
    {
      model: "soniqo-parakeet-streaming",
      languages: ["de"],
      expected: "live",
    },
  ] as const)(
    "returns $expected for $model with $languages",
    ({ model, languages, expected }) => {
      expect(getOnDeviceTranscriptionMode(model, languages)).toBe(expected);
    },
  );
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
    for (const model of ["qwen3-asr", "qwen3-asr-fast"]) {
      expect(getSttModelTranscriptionMode("nari", model)).toBe("live");
    }
  });
});

describe("isSupportedLocalSttModel", () => {
  test.each([
    { model: "soniqo-parakeet-streaming", expected: true },
    { model: "apple-speech", expected: true },
    { model: "am-parakeet-v3", expected: true },
    { model: "QuantizedSmallEn", expected: true },
    { model: "cloud", expected: false },
    { model: "Llama3p2_3bQ4", expected: false },
    { model: "removed-local-model", expected: false },
  ])("$model -> $expected", ({ model, expected }) => {
    expect(isSupportedLocalSttModel(model)).toBe(expected);
  });
});

describe("isOnDeviceSttModel", () => {
  test.each([
    { provider: "soniqo", model: "soniqo-parakeet-streaming", expected: true },
    { provider: "apple_speech", model: "apple-speech", expected: true },
    { provider: "soniqo", model: "apple-speech", expected: false },
    {
      provider: "apple_speech",
      model: "soniqo-parakeet-batch",
      expected: false,
    },
    { provider: "anarlog", model: "soniqo-parakeet-streaming", expected: true },
  ])("$provider/$model -> $expected", ({ provider, model, expected }) => {
    expect(isOnDeviceSttModel(provider, model)).toBe(expected);
  });
});

describe("isLocalFileSttModel", () => {
  test.each([
    { provider: "local_file", model: "local-file", expected: true },
    { provider: "local_file", model: "ggml-small.bin", expected: false },
    { provider: "anarlog", model: "local-file", expected: false },
  ])("$provider/$model -> $expected", ({ provider, model, expected }) => {
    expect(isLocalFileSttModel(provider, model)).toBe(expected);
  });
});

describe("isRealtimeLocalModel", () => {
  test.each([
    { model: "soniqo-parakeet-streaming", expected: true },
    { model: "apple-speech", expected: true },
    { model: "soniqo-parakeet-batch", expected: false },
    { model: "am-parakeet-v3", expected: false },
  ])("$model -> $expected", ({ model, expected }) => {
    expect(isRealtimeLocalModel(model)).toBe(expected);
  });
});

describe("isConfiguredSttModel", () => {
  test.each([
    { provider: "anarlog", model: "cloud", expected: true },
    { provider: "anarlog", model: "soniqo-qwen3-small", expected: true },
    { provider: "anarlog", model: "removed-local-model", expected: false },
    { provider: "soniqo", model: "soniqo-parakeet-batch", expected: true },
    { provider: "soniqo", model: "apple-speech", expected: false },
    { provider: "apple_speech", model: "apple-speech", expected: true },
    {
      provider: "apple_speech",
      model: "soniqo-parakeet-streaming",
      expected: false,
    },
    { provider: "local_file", model: "local-file", expected: true },
    { provider: "local_file", model: "ggml-small.bin", expected: false },
    { provider: "custom", model: "whisper-large-v3", expected: true },
  ])("$provider/$model -> $expected", ({ provider, model, expected }) => {
    expect(isConfiguredSttModel(provider, model)).toBe(expected);
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
  test.each([
    {
      model: "apple-speech",
      languages: ["ko"],
      expected: { languages: ["ko"], transcriptionMode: "live" },
    },
    {
      model: "apple-speech",
      languages: ["ja"],
      expected: { languages: ["ja"], transcriptionMode: "live" },
    },
    {
      model: "apple-speech",
      languages: ["hi"],
      expected: { languages: [], transcriptionMode: "live" },
    },
    {
      model: "soniqo-parakeet-streaming",
      languages: ["en", "ko"],
      expected: { languages: ["en"], transcriptionMode: "live" },
    },
    {
      model: "soniqo-parakeet-streaming",
      languages: ["de", "en"],
      expected: { languages: ["de"], transcriptionMode: "live" },
    },
    {
      model: "soniqo-parakeet-streaming",
      languages: ["ko"],
      expected: { languages: [], transcriptionMode: "live" },
    },
    {
      model: "soniqo-parakeet-streaming",
      languages: ["de"],
      expected: { languages: ["de"], transcriptionMode: "live" },
    },
  ])(
    "$model with $languages -> $expected",
    ({ model, languages, expected }) => {
      expect(getOnDeviceTranscriptionConfig(model, languages)).toEqual(
        expected,
      );
    },
  );
});

describe("getLiveTranscriptionConfig", () => {
  test.each([
    {
      provider: "local_file",
      model: "local-file",
      languages: ["en", "ko"],
      expected: { languages: ["en", "ko"], transcriptionMode: "batch" },
      checksLiveLanguages: false,
    },
    {
      provider: "openai",
      model: "gpt-live-transcribe",
      languages: ["en", "ko"],
      expected: { languages: ["en", "ko"], transcriptionMode: "live" },
      checksLiveLanguages: true,
    },
    {
      provider: "openai",
      model: "gpt-transcribe",
      languages: ["en", "ko"],
      expected: { languages: ["en", "ko"], transcriptionMode: "batch" },
      checksLiveLanguages: false,
    },
    {
      provider: "elevenlabs",
      model: "scribe_v2",
      languages: ["en"],
      expected: { languages: ["en"], transcriptionMode: "batch" },
      checksLiveLanguages: false,
    },
    {
      provider: "elevenlabs",
      model: "scribe_v2_realtime",
      languages: ["en"],
      expected: { languages: ["en"], transcriptionMode: "live" },
      checksLiveLanguages: true,
    },
  ])(
    "resolves $provider/$model to $expected.transcriptionMode",
    async ({ provider, model, languages, expected, checksLiveLanguages }) => {
      await expect(
        getLiveTranscriptionConfig({ provider, model, languages }),
      ).resolves.toEqual(expected);
      if (!checksLiveLanguages) {
        expect(isSupportedLanguagesLiveMock).not.toHaveBeenCalled();
      }
    },
  );

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

  test.each([
    {
      name: "custom providers check as Deepgram-compatible for language fallback",
      mockLive: true,
      invoke: () =>
        getLiveTranscriptionConfig({
          provider: "custom",
          model: "nova-3-general",
          languages: ["en", "ko"],
        }),
      source: "live" as const,
      expected: ["deepgram"],
    },
    {
      name: "Cloudflare Workers AI checks as Deepgram-compatible for language fallback",
      mockLive: false,
      invoke: () =>
        getLiveTranscriptionConfig({
          provider: "cloudflare_workers_ai",
          model: "nova-3",
          languages: ["en", "ko"],
        }),
      source: "live" as const,
      expected: ["deepgram"],
    },
    {
      name: "Cloudflare Workers AI checks as Deepgram-compatible for live language support",
      mockLive: false,
      invoke: () =>
        isSupportedLanguagesLive("cloudflare_workers_ai", "nova-3", ["en"]),
      source: "live" as const,
      expected: ["deepgram", "nova-3", ["en"]],
    },
    {
      name: "the Apple Speech product provider maps to its runtime provider",
      mockLive: false,
      invoke: () =>
        isSupportedLanguagesLive("apple_speech", "apple-speech", ["ko"]),
      source: "live" as const,
      expected: ["apple-speech", "apple-speech", ["ko"]],
    },
    {
      name: "Cloudflare Workers AI checks as Deepgram-compatible for batch language support",
      mockLive: false,
      invoke: () =>
        isSupportedLanguagesBatch("cloudflare_workers_ai", "nova-3", ["en"]),
      source: "batch" as const,
      expected: ["deepgram", "nova-3", ["en"]],
    },
  ])("$name", async ({ mockLive, invoke, source, expected }) => {
    if (mockLive) {
      isSupportedLanguagesLiveMock.mockImplementation(
        (_provider, _model, languages) =>
          Promise.resolve({
            status: "ok" as const,
            data: languages.length === 1 && languages[0] === "en",
          }),
      );
    }

    await invoke();

    const mock =
      source === "live"
        ? isSupportedLanguagesLiveMock
        : isSupportedLanguagesBatchMock;
    expected.forEach((value, index) => {
      expect(mock.mock.calls[0]?.[index]).toEqual(value);
    });
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
