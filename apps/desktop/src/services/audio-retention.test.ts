import { describe, expect, test } from "vitest";

import {
  normalizeAudioRetention,
  sessionAudioExpired,
} from "./audio-retention";

describe("audio retention", () => {
  test("normalizes current and legacy values", () => {
    expect(normalizeAudioRetention("none")).toBe("none");
    expect(normalizeAudioRetention("oneWeek")).toBe("oneWeek");
    expect(normalizeAudioRetention(false)).toBe("none");
    expect(normalizeAudioRetention(true)).toBe("oneMonth");
    expect(normalizeAudioRetention("invalid")).toBe("oneMonth");
    expect(normalizeAudioRetention("invalid", undefined)).toBeUndefined();
  });

  test("expires immediately when retention is none", () => {
    expect(sessionAudioExpired("not-a-date", "none")).toBe(true);
  });

  test("expires after the selected retention window", () => {
    const now = Date.parse("2026-05-13T00:00:00.000Z");

    expect(sessionAudioExpired("2026-05-11T23:59:59.999Z", "oneDay", now)).toBe(
      true,
    );
    expect(sessionAudioExpired("2026-05-12T00:00:00.001Z", "oneDay", now)).toBe(
      false,
    );
  });

  test("does not expire sessions with invalid creation dates", () => {
    expect(sessionAudioExpired(null, "oneDay")).toBe(false);
    expect(sessionAudioExpired("not-a-date", "oneDay")).toBe(false);
  });
});
