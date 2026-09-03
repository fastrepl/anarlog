import { describe, expect, test } from "vitest";

import {
  getDefaultSttModel,
  getDefaultSttSelection,
  getLanguageSupportIssue,
  getPreferredProviderModel,
  resolveLiveLanguageSupportMode,
} from "./selection";

import { normalizeStoredSttModel } from "~/stt/model-selection";

describe("normalizeStoredSttModel", () => {
  test("rewrites retired provider model ids to their replacements", () => {
    expect(normalizeStoredSttModel("assemblyai", "universal-3-pro")).toBe(
      "universal-3-5-pro",
    );
    expect(normalizeStoredSttModel("assemblyai", "u3-rt-pro")).toBe(
      "universal-3-5-pro-realtime",
    );
    expect(normalizeStoredSttModel("aquavoice", "avalon-v1-en")).toBe(
      "avalon-v1.5",
    );
    expect(normalizeStoredSttModel("aquavoice", "avalon-v1.5")).toBe(
      "avalon-v1.5",
    );
    expect(normalizeStoredSttModel("soniox", "stt-rt-v4")).toBe("stt-rt-v5");
    expect(normalizeStoredSttModel("soniox", "stt-async-v3")).toBe("stt-rt-v5");
    expect(normalizeStoredSttModel("openai", "gpt-4o-mini-transcribe")).toBe(
      "gpt-transcribe",
    );
    expect(
      normalizeStoredSttModel("openrouter", "openai/gpt-4o-transcribe"),
    ).toBe("openai/gpt-transcribe");
    expect(normalizeStoredSttModel("openai", "whisper-1")).toBe("whisper-1");
    expect(normalizeStoredSttModel("deepgram", "nova-3-general")).toBe(
      "nova-3-general",
    );
  });
});

describe("getDefaultSttModel", () => {
  test("repairs external providers with their first supported model", () => {
    expect(getDefaultSttModel("local_file")).toBe("local-file");
    expect(getDefaultSttModel("deepgram")).toBe("nova-3-general");
    expect(getDefaultSttModel("assemblyai")).toBe("universal-3-5-pro");
    expect(getDefaultSttModel("soniox")).toBe("stt-rt-v5");
    expect(getDefaultSttModel("cohere")).toBe("cohere-transcribe-03-2026");
    expect(getDefaultSttModel("aquavoice")).toBe("avalon-v1.5");
    expect(getDefaultSttModel("dashscope")).toBe("qwen3-asr-flash-realtime");
    expect(getDefaultSttModel("zai")).toBe("glm-asr-2512");
    expect(getDefaultSttModel("siliconflow")).toBe(
      "FunAudioLLM/SenseVoiceSmall",
    );
    expect(getDefaultSttModel("groq")).toBe("whisper-large-v3-turbo");
    expect(getDefaultSttModel("openrouter")).toBe("openai/gpt-transcribe");
    expect(getDefaultSttModel("xai")).toBe("xai-stt");
    expect(getDefaultSttModel("smallestai")).toBe("pulse");
    expect(getDefaultSttModel("google_generative_ai")).toBe(
      "gemini-3.5-transcribe-live",
    );
    expect(getDefaultSttModel("together")).toBe("openai/whisper-large-v3");
    expect(getDefaultSttModel("speechmatics")).toBe("enhanced");
    expect(getDefaultSttModel("azure_speech")).toBe("fast-transcription");
    expect(getDefaultSttModel("google_cloud")).toBe("latest_long");
    expect(getDefaultSttModel("google_generative_ai")).toBe(
      "gemini-3.5-transcribe-live",
    );
    expect(getDefaultSttModel("aws_transcribe")).toBe("amazon-transcribe");
    expect(getDefaultSttModel("revai")).toBe("machine");
    expect(getDefaultSttModel("fireworks")).toBe("whisper-v3-turbo");
  });

  test("does not invent a model for custom or Anarlog providers", () => {
    expect(getDefaultSttModel("custom")).toBeUndefined();
    expect(getDefaultSttModel("anarlog")).toBeUndefined();
  });
});

describe("getPreferredProviderModel", () => {
  test("returns the remembered model when it is still available", () => {
    expect(
      getPreferredProviderModel("nova-2-meeting", [
        { id: "nova-3-general" },
        { id: "nova-2-meeting" },
      ]),
    ).toBe("nova-2-meeting");
  });

  test("falls back to the first available model when none is remembered", () => {
    expect(
      getPreferredProviderModel(undefined, [
        { id: "stt-rt-v5" },
        { id: "stt-rt-v4" },
      ]),
    ).toBe("stt-rt-v5");
  });

  test("falls back to the first available model when the remembered model is gone", () => {
    expect(
      getPreferredProviderModel("nova-2-meeting", [
        { id: "nova-3-general" },
        { id: "nova-2-general" },
      ]),
    ).toBe("nova-3-general");
  });

  test("skips models that are not selectable", () => {
    expect(
      getPreferredProviderModel(undefined, [
        { id: "cloud", isDownloaded: false },
        { id: "soniqo-qwen3-small", isDownloaded: true },
      ]),
    ).toBe("soniqo-qwen3-small");
  });

  test("can keep a saved model visible even when it is not selectable", () => {
    expect(
      getPreferredProviderModel(
        "cloud",
        [
          { id: "cloud", isDownloaded: false },
          { id: "soniqo-parakeet-streaming", isDownloaded: true },
        ],
        { keepUnavailableSavedModel: true },
      ),
    ).toBe("cloud");
  });

  test("clears the selection when a provider has no selectable models", () => {
    expect(
      getPreferredProviderModel("cloud", [
        { id: "cloud", isDownloaded: false },
      ]),
    ).toBe("");
  });

  test("migrates retired AssemblyAI models to Universal 3.5 Pro", () => {
    const models = [
      { id: "universal-3-5-pro" },
      { id: "universal-3-5-pro-realtime" },
    ];

    expect(getPreferredProviderModel("universal", models)).toBe(
      "universal-3-5-pro",
    );
    expect(getPreferredProviderModel("universal-3-pro", models)).toBe(
      "universal-3-5-pro",
    );
    expect(getPreferredProviderModel("u3-rt-pro", models)).toBe(
      "universal-3-5-pro-realtime",
    );
  });

  test("migrates the retired AquaVoice model to Avalon 1.5", () => {
    expect(
      getPreferredProviderModel("avalon-v1-en", [{ id: "avalon-v1.5" }]),
    ).toBe("avalon-v1.5");
  });

  test("migrates every Soniox alias to the v5 realtime model", () => {
    for (const saved of ["stt-v5", "stt-async-v4", "stt-rt-v4", "stt-rt-v3"]) {
      expect(getPreferredProviderModel(saved, [{ id: "stt-rt-v5" }])).toBe(
        "stt-rt-v5",
      );
    }
  });

  test("migrates the superseded OpenAI transcribe models to gpt-transcribe", () => {
    const openai = [{ id: "gpt-transcribe" }, { id: "whisper-1" }];
    expect(getPreferredProviderModel("gpt-4o-transcribe", openai)).toBe(
      "gpt-transcribe",
    );
    expect(getPreferredProviderModel("gpt-4o-mini-transcribe", openai)).toBe(
      "gpt-transcribe",
    );
    expect(
      getPreferredProviderModel("openai/gpt-4o-mini-transcribe", [
        { id: "openai/gpt-transcribe" },
      ]),
    ).toBe("openai/gpt-transcribe");
  });

  test("keeps the remembered value when the provider does not expose a static list", () => {
    expect(
      getPreferredProviderModel("whisper-large-v3", [], {
        allowSavedModelWithoutChoices: true,
      }),
    ).toBe("whisper-large-v3");
  });
});

describe("getDefaultSttSelection", () => {
  test("keeps the active configured provider and repairs its missing model", () => {
    expect(
      getDefaultSttSelection(
        ["deepgram", "assemblyai"],
        {
          deepgram: {
            configured: true,
            models: [{ id: "nova-3-general" }],
          },
          assemblyai: {
            configured: true,
            models: [{ id: "universal-3-5-pro" }],
          },
        },
        "deepgram",
      ),
    ).toEqual({ provider: "deepgram", model: "nova-3-general" });
  });

  test("skips configured providers that have no available model", () => {
    expect(
      getDefaultSttSelection(["anarlog", "deepgram"], {
        anarlog: {
          configured: true,
          models: [{ id: "cloud", isDownloaded: false }],
        },
        deepgram: {
          configured: true,
          models: [{ id: "nova-3-general" }],
        },
      }),
    ).toEqual({ provider: "deepgram", model: "nova-3-general" });
  });

  test("returns no selection when nothing is available", () => {
    expect(
      getDefaultSttSelection(["anarlog"], {
        anarlog: {
          configured: true,
          models: [{ id: "cloud", isDownloaded: false }],
        },
      }),
    ).toBeNull();
  });
});

describe("getLanguageSupportIssue", () => {
  test("returns the languages the model cannot transcribe", async () => {
    const issue = await getLanguageSupportIssue(
      "en",
      ["ko", "ja"],
      async (languages) => !languages.includes("ko"),
    );

    expect(issue).toEqual({ unsupportedLanguages: ["ko"] });
  });

  test("distinguishes an unsupported combination from unsupported languages", async () => {
    const issue = await getLanguageSupportIssue(
      "en",
      ["ko"],
      async (languages) => languages.length === 1,
    );

    expect(issue).toEqual({ unsupportedLanguages: [] });
  });

  test("returns no issue when the full selection is supported", async () => {
    const issue = await getLanguageSupportIssue("en", ["ko"], async () => true);

    expect(issue).toBeNull();
  });
});

describe("resolveLiveLanguageSupportMode", () => {
  test("uses provider live support for hosted models", () => {
    expect(
      resolveLiveLanguageSupportMode({
        isOnDeviceModel: false,
        useLiveOnDeviceModel: false,
        liveSupported: true,
      }),
    ).toBe(true);
  });

  test("keeps batch-only on-device models in batch mode", () => {
    expect(
      resolveLiveLanguageSupportMode({
        isOnDeviceModel: true,
        useLiveOnDeviceModel: false,
        liveSupported: true,
      }),
    ).toBe(false);
  });

  test("requires provider live support for realtime on-device models", () => {
    expect(
      resolveLiveLanguageSupportMode({
        isOnDeviceModel: true,
        useLiveOnDeviceModel: true,
        liveSupported: false,
      }),
    ).toBe(false);
  });
});
