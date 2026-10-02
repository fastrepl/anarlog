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
    expect(normalizeStoredSttModel("nari", "qwen3-asr-fast:free")).toBe(
      "qwen3-asr-fast",
    );
    expect(normalizeStoredSttModel("nari", "qwen3-asr:free")).toBe("qwen3-asr");
    expect(normalizeStoredSttModel("nari", "qwen3-asr")).toBe("qwen3-asr");
    expect(normalizeStoredSttModel("openai", "whisper-1")).toBe("whisper-1");
    expect(normalizeStoredSttModel("deepgram", "nova-3-general")).toBe(
      "nova-3-general",
    );
  });
});

describe("getDefaultSttModel", () => {
  test.each([
    ["local_file", "local-file"],
    ["deepgram", "nova-3-general"],
    ["assemblyai", "universal-3-5-pro"],
    ["custom", undefined],
    ["anarlog", undefined],
  ])("defaults %s to %s", (provider, model) => {
    expect(getDefaultSttModel(provider)).toBe(model);
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

  test.each([
    {
      saved: "qwen3-asr:free",
      models: [{ id: "qwen3-asr-fast" }, { id: "qwen3-asr" }],
      expected: "qwen3-asr",
    },
    {
      saved: "qwen3-asr-fast:free",
      models: [{ id: "qwen3-asr-fast" }, { id: "qwen3-asr" }],
      expected: "qwen3-asr-fast",
    },
    {
      saved: "universal",
      models: [
        { id: "universal-3-5-pro" },
        { id: "universal-3-5-pro-realtime" },
      ],
      expected: "universal-3-5-pro",
    },
    {
      saved: "universal-3-pro",
      models: [
        { id: "universal-3-5-pro" },
        { id: "universal-3-5-pro-realtime" },
      ],
      expected: "universal-3-5-pro",
    },
    {
      saved: "u3-rt-pro",
      models: [
        { id: "universal-3-5-pro" },
        { id: "universal-3-5-pro-realtime" },
      ],
      expected: "universal-3-5-pro-realtime",
    },
    {
      saved: "avalon-v1-en",
      models: [{ id: "avalon-v1.5" }],
      expected: "avalon-v1.5",
    },
    {
      saved: "stt-v5",
      models: [{ id: "stt-rt-v5" }],
      expected: "stt-rt-v5",
    },
    {
      saved: "stt-async-v4",
      models: [{ id: "stt-rt-v5" }],
      expected: "stt-rt-v5",
    },
    {
      saved: "stt-rt-v4",
      models: [{ id: "stt-rt-v5" }],
      expected: "stt-rt-v5",
    },
    {
      saved: "stt-rt-v3",
      models: [{ id: "stt-rt-v5" }],
      expected: "stt-rt-v5",
    },
    {
      saved: "gpt-4o-transcribe",
      models: [{ id: "gpt-transcribe" }, { id: "whisper-1" }],
      expected: "gpt-transcribe",
    },
    {
      saved: "gpt-4o-mini-transcribe",
      models: [{ id: "gpt-transcribe" }, { id: "whisper-1" }],
      expected: "gpt-transcribe",
    },
    {
      saved: "openai/gpt-4o-mini-transcribe",
      models: [{ id: "openai/gpt-transcribe" }],
      expected: "openai/gpt-transcribe",
    },
  ])(
    "migrates retired model $saved to $expected",
    ({ saved, models, expected }) => {
      expect(getPreferredProviderModel(saved, models)).toBe(expected);
    },
  );

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
