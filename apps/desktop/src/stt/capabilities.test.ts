import { beforeEach, describe, expect, test, vi } from "vitest";

const { isSupportedLanguagesBatchMock, isSupportedLanguagesLiveMock } =
  vi.hoisted(() => ({
    isSupportedLanguagesBatchMock: vi.fn(),
    isSupportedLanguagesLiveMock: vi.fn(),
  }));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: {
    isSupportedLanguagesBatch: isSupportedLanguagesBatchMock,
    isSupportedLanguagesLive: isSupportedLanguagesLiveMock,
  },
}));

import {
  getLiveTranscriptionConfig,
  getOnDeviceTranscriptionConfig,
  getOnDeviceTranscriptionMode,
  getTranscriptionLanguages,
  isConfiguredSttModel,
  isSupportedLanguagesBatch,
  isSupportedLanguagesLive,
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

  test("falls back to batch for non-English Soniqo streaming languages", () => {
    expect(
      getOnDeviceTranscriptionMode("soniqo-parakeet-streaming", ["de"]),
    ).toBe("batch");
  });
});

describe("isSupportedLocalSttModel", () => {
  test("accepts shipped local STT model families", () => {
    expect(isSupportedLocalSttModel("soniqo-parakeet-streaming")).toBe(true);
    expect(isSupportedLocalSttModel("am-parakeet-v3")).toBe(true);
    expect(isSupportedLocalSttModel("QuantizedSmallEn")).toBe(true);
  });

  test("rejects cloud, local LLM, and removed local model ids", () => {
    expect(isSupportedLocalSttModel("cloud")).toBe(false);
    expect(isSupportedLocalSttModel("Llama3p2_3bQ4")).toBe(false);
    expect(isSupportedLocalSttModel("removed-local-model")).toBe(false);
  });
});

describe("isConfiguredSttModel", () => {
  test("requires known model ids for Anarlog STT", () => {
    expect(isConfiguredSttModel("hyprnote", "cloud")).toBe(true);
    expect(isConfiguredSttModel("hyprnote", "soniqo-qwen3-small")).toBe(true);
    expect(isConfiguredSttModel("hyprnote", "removed-local-model")).toBe(false);
  });

  test("allows custom model ids for external providers", () => {
    expect(isConfiguredSttModel("custom", "whisper-large-v3")).toBe(true);
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

  test("keeps German on batch even when English is an additional language", () => {
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["de", "en"]),
    ).toEqual({
      languages: ["de", "en"],
      transcriptionMode: "batch",
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
