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

  test("keeps recording disclosure auto-post off until explicitly enabled", () => {
    expect(
      resolveConfigValue("consent_auto_send_chat", {
        values: {},
        hasValues: new Set(),
      }),
    ).toBe(false);
  });

  test("keeps automatic updates on until explicitly disabled", () => {
    expect(
      resolveConfigValue("automatic_updates", {
        values: {},
        hasValues: new Set(),
      }),
    ).toBe(true);
  });

  test("uses disabled legacy bounce preferences for the general setting", () => {
    expect(
      resolveConfigValue("notification_bounce", {
        values: {
          notification_bounce_summary: true,
          notification_bounce_transcript: false,
        },
        hasValues: new Set([
          "notification_bounce_summary",
          "notification_bounce_transcript",
        ]),
      }),
    ).toBe(false);
  });

  test("prefers the explicit general bounce preference", () => {
    expect(
      resolveConfigValue("notification_bounce", {
        values: {
          notification_bounce: true,
          notification_bounce_summary: false,
        },
        hasValues: new Set([
          "notification_bounce",
          "notification_bounce_summary",
        ]),
      }),
    ).toBe(true);
  });
});
