import { describe, expect, test } from "vitest";

import { getPreferredProviderModel } from "./selection";

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
        { id: "stt-v5" },
        { id: "stt-v4" },
      ]),
    ).toBe("stt-v5");
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

  test("keeps the remembered value when the provider does not expose a static list", () => {
    expect(
      getPreferredProviderModel("whisper-large-v3", [], {
        allowSavedModelWithoutChoices: true,
      }),
    ).toBe("whisper-large-v3");
  });
});
