import { describe, expect, test } from "vitest";

import { resolveConfigValue } from ".";

describe("resolveConfigValue", () => {
  test("uses legacy don't-save when audio retention is missing", () => {
    expect(
      resolveConfigValue("audio_retention", {
        values: { save_recordings: false },
        hasValues: new Set(["save_recordings"]),
      }),
    ).toBe("none");
  });

  test("keeps explicit audio retention over legacy save_recordings", () => {
    expect(
      resolveConfigValue("audio_retention", {
        values: { save_recordings: false, audio_retention: "oneMonth" },
        hasValues: new Set(["save_recordings", "audio_retention"]),
      }),
    ).toBe("oneMonth");
  });

  test("parses stored array values without exposing malformed entries", () => {
    expect(
      resolveConfigValue("spoken_languages", {
        values: { spoken_languages: '["en",2,"ko"]' },
        hasValues: new Set(["spoken_languages"]),
      }),
    ).toEqual(["en", "ko"]);
  });
});
