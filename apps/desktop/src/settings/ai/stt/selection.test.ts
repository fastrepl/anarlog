import { describe, expect, test } from "vitest";

import {
  getDefaultSttModel,
  getPreferredProviderModel,
  resolveLiveLanguageSupportMode,
} from "./selection";

describe("getDefaultSttModel", () => {
  test("repairs external providers with their first supported model", () => {
    expect(getDefaultSttModel("deepgram")).toBe("nova-3-general");
    expect(getDefaultSttModel("soniox")).toBe("stt-rt-v5");
  });

  test("does not invent a model for custom or Anarlog providers", () => {
    expect(getDefaultSttModel("custom")).toBeUndefined();
    expect(getDefaultSttModel("hyprnote")).toBeUndefined();
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

  test("migrates AssemblyAI universal to universal-3-pro when available", () => {
    expect(
      getPreferredProviderModel("universal", [
        { id: "universal-3-pro" },
        { id: "universal-2" },
      ]),
    ).toBe("universal-3-pro");
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
