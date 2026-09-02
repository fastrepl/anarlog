import { describe, expect, test } from "vitest";

import {
  getDefaultSttModel,
  getDefaultSttSelection,
  getLanguageSupportIssue,
  getPreferredProviderModel,
  resolveLiveLanguageSupportMode,
} from "./selection";

describe("getDefaultSttModel", () => {
  test("repairs external providers with their first supported model", () => {
    expect(getDefaultSttModel("local_file")).toBe("local-file");
    expect(getDefaultSttModel("deepgram")).toBe("nova-3-general");
    expect(getDefaultSttModel("assemblyai")).toBe("universal-3-5-pro");
    expect(getDefaultSttModel("soniox")).toBe("stt-rt-v5");
    expect(getDefaultSttModel("cohere")).toBe("cohere-transcribe-03-2026");
    expect(getDefaultSttModel("groq")).toBe("whisper-large-v3-turbo");
    expect(getDefaultSttModel("openrouter")).toBe("openai/gpt-transcribe");
    expect(getDefaultSttModel("xai")).toBe("xai-stt");
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

  test("migrates AssemblyAI universal to universal-3-5-pro when available", () => {
    expect(
      getPreferredProviderModel("universal", [
        { id: "universal-3-5-pro" },
        { id: "universal-3-pro" },
      ]),
    ).toBe("universal-3-5-pro");
  });

  test("migrates Soniox aliases to explicit realtime models", () => {
    expect(
      getPreferredProviderModel("stt-v5", [
        { id: "stt-rt-v5" },
        { id: "stt-rt-v4" },
      ]),
    ).toBe("stt-rt-v5");

    expect(
      getPreferredProviderModel("stt-async-v4", [
        { id: "stt-rt-v5" },
        { id: "stt-rt-v4" },
      ]),
    ).toBe("stt-rt-v4");
  });

  test("migrates removed Soniox v3 aliases to v4 realtime", () => {
    expect(
      getPreferredProviderModel("stt-rt-v3", [
        { id: "stt-rt-v5" },
        { id: "stt-rt-v4" },
      ]),
    ).toBe("stt-rt-v4");
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
