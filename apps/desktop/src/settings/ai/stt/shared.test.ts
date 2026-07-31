import { describe, expect, test } from "vitest";

import { displayModelLabel, PROVIDERS } from "./shared";

describe("STT model display labels", () => {
  test("keeps cloud model product-facing", () => {
    expect(displayModelLabel("cloud")).toBe("Pro (Cloud)");
  });

  test("uses product-facing labels for hosted provider models", () => {
    expect(displayModelLabel("stt-rt-v5")).toBe("Soniox 5");
    expect(displayModelLabel("u3-rt-pro")).toBe("Universal 3.5 Pro Realtime");
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
    expect(displayModelLabel("fast-transcription")).toBe("Fast Transcription");
  });

  test("exposes all new providers with honest capability badges", () => {
    const providers = Object.fromEntries(
      PROVIDERS.map((provider) => [provider.id, provider]),
    );

    expect(providers.fireworks.disabled).toBe(false);
    expect(providers.fireworks.models).toEqual(["whisper-v3-turbo"]);
    expect(providers.xai.badge).toBeNull();
    for (const provider of [
      "groq",
      "together",
      "speechmatics",
      "azure_speech",
      "revai",
    ]) {
      expect(providers[provider]?.badge).toBe("Batch only");
    }
    expect(providers.google_cloud.badge).toBe("Short batch");
    expect(providers.aws_transcribe.badge).toBe("Gateway");
  });

  test("names on-device models instead of collapsing them", () => {
    expect(displayModelLabel("apple-speech", "Apple Speech")).toBe(
      "Apple Speech",
    );
    expect(
      displayModelLabel(
        "soniqo-parakeet-streaming",
        "Soniqo Parakeet Streaming",
      ),
    ).toBe("Soniqo Parakeet Streaming");
  });

  test("names on-device models without a backend display name", () => {
    expect(displayModelLabel("apple-speech")).toBe("Apple Speech");
    expect(displayModelLabel("soniqo-parakeet-batch")).toBe(
      "Soniqo Parakeet Batch",
    );
    expect(displayModelLabel("soniqo-omnilingual")).toBe("Soniqo Omnilingual");
  });
});
