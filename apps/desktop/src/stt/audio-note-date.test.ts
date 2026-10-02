import { describe, expect, it } from "vitest";

import { estimateUploadedAudioSessionCreatedAt } from "./audio-note-date";

describe("estimateUploadedAudioSessionCreatedAt", () => {
  it.each([
    {
      name: "uses createdAt and subtracts duration",
      input: {
        createdAt: "2026-03-26T12:00:00.000Z",
        modifiedAt: "2026-03-26T11:00:00.000Z",
        durationMs: 30_000,
      },
      expected: "2026-03-26T11:59:30.000Z",
    },
    {
      name: "falls back to modifiedAt",
      input: {
        createdAt: null,
        modifiedAt: "2026-03-26T12:00:00.000Z",
        durationMs: 5_000,
      },
      expected: "2026-03-26T11:59:55.000Z",
    },
    {
      name: "uses the anchor timestamp when duration is missing",
      input: {
        createdAt: "2026-03-26T12:00:00.000Z",
        modifiedAt: null,
        durationMs: null,
      },
      expected: "2026-03-26T12:00:00.000Z",
    },
    {
      name: "returns null when no valid timestamp exists",
      input: {
        createdAt: null,
        modifiedAt: null,
        durationMs: 10_000,
      },
      expected: null,
    },
  ])("$name", ({ input, expected }) => {
    expect(estimateUploadedAudioSessionCreatedAt(input)).toBe(expected);
  });
});
