import { describe, expect, test } from "vitest";

import { displayModelLabel, displayModelTitle } from "./shared";

describe("STT model display labels", () => {
  test("keeps cloud model product-facing", () => {
    expect(displayModelLabel("cloud")).toBe("Pro (Cloud)");
    expect(displayModelTitle("cloud")).toBeUndefined();
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
  });

  test("treats apple speech as an on-device model", () => {
    expect(displayModelLabel("apple-speech", "Apple Speech")).toBe("On device");
    expect(displayModelTitle("apple-speech", "Apple Speech")).toBe(
      "Apple Speech",
    );
  });

  test("collapses local model names to on-device labels", () => {
    expect(
      displayModelLabel(
        "soniqo-parakeet-streaming",
        "Soniqo Parakeet Streaming",
      ),
    ).toBe("On device");
    expect(
      displayModelTitle(
        "soniqo-parakeet-streaming",
        "Soniqo Parakeet Streaming",
      ),
    ).toBe("Soniqo Parakeet Streaming");
  });
});
