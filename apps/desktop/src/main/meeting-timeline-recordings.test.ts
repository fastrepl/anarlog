import { describe, expect, test } from "vitest";

import { buildSessionRecordingRanges } from "./meeting-timeline-recordings";

describe("meeting timeline recording ranges", () => {
  test("derives recording end from transcript word timing", () => {
    const startedAt = new Date("2026-05-27T10:51:00.000Z").getTime();
    const ranges = buildSessionRecordingRanges({
      transcript: {
        session_id: "session",
        started_at: startedAt,
        words: JSON.stringify([{ end_ms: 98_000 }]),
      },
    });

    expect(ranges.get("session")).toEqual({
      start: new Date(startedAt),
      end: new Date(startedAt + 98_000),
    });
  });

  test("merges multiple transcript rows for a session", () => {
    const firstStart = new Date("2026-05-27T10:00:00.000Z").getTime();
    const secondStart = new Date("2026-05-27T10:04:00.000Z").getTime();
    const ranges = buildSessionRecordingRanges({
      first: {
        session_id: "session",
        started_at: firstStart,
        ended_at: firstStart + 60_000,
      },
      second: {
        session_id: "session",
        started_at: secondStart,
        words: JSON.stringify([{ end_ms: 30_000 }]),
      },
    });

    expect(ranges.get("session")).toEqual({
      start: new Date(firstStart),
      end: new Date(secondStart + 30_000),
    });
  });
});
