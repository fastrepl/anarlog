import { describe, expect, test } from "vitest";

import { getOnDeviceTranscriptionMode } from "./capabilities";

describe("getOnDeviceTranscriptionMode", () => {
  test("uses live mode for realtime local models", () => {
    expect(getOnDeviceTranscriptionMode("soniqo-parakeet-streaming")).toBe(
      "live",
    );
  });

  test("uses batch mode for non-realtime local models", () => {
    expect(
      getOnDeviceTranscriptionMode("cactus-parakeet-tdt-0.6b-v3-int8"),
    ).toBe("batch");
  });
});
