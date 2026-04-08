import { describe, expect, test } from "vitest";

import { getBatchProvider } from "./useRunBatch";

describe("getBatchProvider", () => {
  test("maps pyannote to the batch transcription provider", () => {
    expect(getBatchProvider("pyannote", "parakeet-tdt-0.6b-v3")).toBe(
      "pyannote",
    );
  });

  test("keeps openai mapped to the batch transcription provider", () => {
    expect(getBatchProvider("openai", "gpt-4o-transcribe")).toBe("openai");
  });
});
