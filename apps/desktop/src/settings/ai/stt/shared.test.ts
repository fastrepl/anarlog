import { describe, expect, test } from "vitest";

import { isDeprecatedSttModel, PROVIDERS } from "./shared";

describe("STT model deprecation", () => {
  test("only keeps deprecated models that have no replacement", () => {
    expect(isDeprecatedSttModel("openai", "gpt-4o-transcribe-diarize")).toBe(
      true,
    );
    expect(isDeprecatedSttModel("openai", "whisper-1")).toBe(true);
    expect(isDeprecatedSttModel("openai", "gpt-transcribe")).toBe(false);
    expect(isDeprecatedSttModel("openrouter", "openai/gpt-transcribe")).toBe(
      false,
    );
    expect(isDeprecatedSttModel("soniox", "stt-rt-v5")).toBe(false);
  });

  test("only lists models the provider APIs still accept", () => {
    const assemblyai = PROVIDERS.find(
      (provider) => provider.id === "assemblyai",
    );
    const providers = Object.fromEntries(
      PROVIDERS.map((provider) => [provider.id, provider]),
    );
    const nari = PROVIDERS.find(({ id }) => id === "nari")!;

    expect(assemblyai?.models).toEqual([
      "universal-3-5-pro",
      "universal-3-5-pro-realtime",
    ]);

    expect(providers.openai.models).toEqual([
      "gpt-live-transcribe",
      "gpt-transcribe",
      "gpt-4o-transcribe-diarize",
      "whisper-1",
    ]);
    expect(providers.openrouter.models).not.toContain(
      "openai/gpt-4o-transcribe",
    );
    expect(providers.openrouter.models).not.toContain(
      "openai/gpt-4o-mini-transcribe",
    );
    expect(providers.soniox.models).toEqual(["stt-rt-v5"]);
    expect(nari.disabled).toBe(false);
    expect(nari.baseUrl).toBe("https://api.narilabs.com");
    expect(nari.models).toEqual(["qwen3-asr-fast", "qwen3-asr"]);
  });
});
