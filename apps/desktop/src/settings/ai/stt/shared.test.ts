import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import {
  displayModelLabel,
  formatDownloadProgress,
  isDeprecatedSttModel,
  PROVIDERS,
} from "./shared";

describe("STT providers", () => {
  test("orders providers by popularity", () => {
    expect(PROVIDERS.map(({ id }) => id)).toEqual([
      "anarlog",
      "soniqo",
      "apple_speech",
      "local_file",
      "deepgram",
      "assemblyai",
      "openai",
      "openrouter",
      "dashscope",
      "zai",
      "siliconflow",
      "google_generative_ai",
      "google_cloud",
      "aws_transcribe",
      "azure_speech",
      "elevenlabs",
      "soniox",
      "speechmatics",
      "groq",
      "mistral",
      "revai",
      "gladia",
      "cartesia",
      "cloudflare_workers_ai",
      "together",
      "fireworks",
      "xai",
      "pyannote",
      "cohere",
      "aquavoice",
      "custom",
    ]);
  });

  test("bundles every provider icon", () => {
    for (const { icon } of PROVIDERS) {
      const markup = renderToStaticMarkup(icon);

      expect(markup).toMatch(/<(img|svg)\b/);
      expect(markup).not.toContain("iconify-icon");
    }
  });
});

describe("STT model display labels", () => {
  test("keeps cloud model product-facing", () => {
    expect(displayModelLabel("cloud")).toBe("Pro (Cloud)");
  });

  test("uses product-facing labels for hosted provider models", () => {
    expect(displayModelLabel("stt-rt-v5")).toBe("Soniox 5");
    expect(displayModelLabel("u3-rt-pro")).toBe("Universal 3 Pro Realtime");
    expect(displayModelLabel("universal-3-pro")).toBe("Universal 3 Pro");
    expect(displayModelLabel("universal-3-5-pro")).toBe("Universal 3.5 Pro");
    expect(displayModelLabel("universal-3-5-pro-realtime")).toBe(
      "Universal 3.5 Pro Realtime",
    );
    expect(displayModelLabel("gpt-4o-transcribe-diarize")).toBe(
      "GPT-4o Transcribe Diarize",
    );
    expect(displayModelLabel("gpt-live-transcribe")).toBe(
      "GPT Live Transcribe",
    );
    expect(displayModelLabel("gpt-transcribe")).toBe("GPT Transcribe");
    expect(displayModelLabel("cohere-transcribe-03-2026")).toBe(
      "Cohere Transcribe",
    );
    expect(displayModelLabel("whisper-large-v3-turbo")).toBe(
      "Whisper Large V3 Turbo",
    );
    expect(displayModelLabel("xai-stt")).toBe("xAI Speech to Text");
    expect(displayModelLabel("gemini-3.5-transcribe-live")).toBe(
      "3.5 Transcribe Live",
    );
    expect(displayModelLabel("gemini-3.5-transcribe")).toBe("3.5 Transcribe");
    expect(displayModelLabel("local-file")).toBe("Local model file");
    expect(displayModelLabel("fast-transcription")).toBe("Fast Transcription");
    expect(displayModelLabel("openai/gpt-4o-mini-transcribe")).toBe(
      "GPT-4o mini Transcribe",
    );
    expect(displayModelLabel("mistralai/voxtral-mini-transcribe")).toBe(
      "Voxtral Mini Transcribe",
    );
    expect(displayModelLabel("qwen3-asr-flash-realtime")).toBe(
      "Qwen3 ASR Flash Realtime",
    );
    expect(displayModelLabel("glm-asr-2512")).toBe("GLM ASR");
    expect(displayModelLabel("FunAudioLLM/SenseVoiceSmall")).toBe(
      "SenseVoice Small",
    );
  });

  test("exposes all new providers with honest capability badges", () => {
    const providers = Object.fromEntries(
      PROVIDERS.map((provider) => [provider.id, provider]),
    );

    expect(providers.fireworks.disabled).toBe(false);
    expect(providers.fireworks.models).toEqual(["whisper-v3-turbo"]);
    expect(providers.xai.badge).toBeNull();
    expect(providers.google_generative_ai.badge).toBeNull();
    expect(providers.google_generative_ai.models).toEqual([
      "gemini-3.5-transcribe-live",
      "gemini-3.5-transcribe",
    ]);
    for (const provider of [
      "groq",
      "openrouter",
      "zai",
      "siliconflow",
      "together",
      "speechmatics",
      "azure_speech",
      "revai",
    ]) {
      expect(providers[provider]?.badge).toBe("Batch only");
    }
    expect(providers.google_cloud.badge).toBe("Short batch");
    expect(providers.aws_transcribe.badge).toBe("Gateway");
    expect(providers.dashscope.badge).toBeNull();
    expect("builtIn" in providers.soniqo && providers.soniqo.builtIn).toBe(
      true,
    );
    expect(
      "builtIn" in providers.apple_speech && providers.apple_speech.builtIn,
    ).toBe(true);
    expect(
      "builtIn" in providers.local_file && providers.local_file.builtIn,
    ).toBe(true);
    expect(providers.local_file.badge).toBe("Batch only");
  });

  test("names on-device models instead of collapsing them", () => {
    expect(displayModelLabel("apple-speech", "Apple Speech")).toBe(
      "Apple Speech",
    );
    expect(
      displayModelLabel("soniqo-parakeet-streaming", "Parakeet Streaming"),
    ).toBe("Parakeet Streaming");
  });

  test("names on-device models without a backend display name", () => {
    expect(displayModelLabel("apple-speech")).toBe("Apple Speech");
    expect(displayModelLabel("soniqo-parakeet-batch")).toBe("Parakeet Batch");
    expect(displayModelLabel("soniqo-omnilingual")).toBe("Omnilingual ASR");
  });

  test("hides unknown or zero download percent", () => {
    expect(formatDownloadProgress(null)).toBeNull();
    expect(formatDownloadProgress(0)).toBeNull();
    expect(formatDownloadProgress(12.4)).toBe("12%");
    expect(formatDownloadProgress(100)).toBe("100%");
  });
});

describe("STT model deprecation", () => {
  test("lists current AssemblyAI models ahead of the Universal 3 pair", () => {
    const assemblyai = PROVIDERS.find(
      (provider) => provider.id === "assemblyai",
    );

    expect(assemblyai?.models).toEqual([
      "universal-3-5-pro",
      "universal-3-5-pro-realtime",
      "universal-3-pro",
      "u3-rt-pro",
    ]);
  });

  test("marks superseded hosted models as deprecated", () => {
    expect(isDeprecatedSttModel("assemblyai", "universal-3-pro")).toBe(true);
    expect(isDeprecatedSttModel("assemblyai", "u3-rt-pro")).toBe(true);
    expect(isDeprecatedSttModel("assemblyai", "universal-3-5-pro")).toBe(false);
    expect(
      isDeprecatedSttModel("assemblyai", "universal-3-5-pro-realtime"),
    ).toBe(false);
    expect(isDeprecatedSttModel("openai", "gpt-4o-transcribe")).toBe(true);
    expect(isDeprecatedSttModel("openai", "gpt-transcribe")).toBe(false);
    expect(
      isDeprecatedSttModel("openrouter", "openai/gpt-4o-mini-transcribe"),
    ).toBe(true);
    expect(isDeprecatedSttModel("openrouter", "openai/gpt-transcribe")).toBe(
      false,
    );
    expect(isDeprecatedSttModel("soniox", "stt-rt-v4")).toBe(true);
    expect(isDeprecatedSttModel("soniox", "stt-rt-v5")).toBe(false);
  });
});
