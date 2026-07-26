import { describe, expect, it } from "vitest";

import { estimateUploadedAudioSessionCreatedAt } from "./audio-note-date";

describe("estimateUploadedAudioSessionCreatedAt", () => {
  it("uses createdAt and subtracts duration", () => {
    expect(
      estimateUploadedAudioSessionCreatedAt({
        createdAt: "2026-03-26T12:00:00.000Z",
        modifiedAt: "2026-03-26T11:00:00.000Z",
        durationMs: 30_000,
      }),
    ).toBe("2026-03-26T11:59:30.000Z");
  });

  it("falls back to modifiedAt", () => {
    expect(
      estimateUploadedAudioSessionCreatedAt({
        createdAt: null,
        modifiedAt: "2026-03-26T12:00:00.000Z",
        durationMs: 5_000,
      }),
    ).toBe("2026-03-26T11:59:55.000Z");
  });

  it("uses the anchor timestamp when duration is missing", () => {
    expect(
      estimateUploadedAudioSessionCreatedAt({
        createdAt: "2026-03-26T12:00:00.000Z",
        modifiedAt: null,
        durationMs: null,
      }),
    ).toBe("2026-03-26T12:00:00.000Z");
  });

  it("returns null when no valid timestamp exists", () => {
    expect(
      estimateUploadedAudioSessionCreatedAt({
        createdAt: null,
        modifiedAt: null,
        durationMs: 10_000,
      }),
    ).toBeNull();
  });
});
