import { describe, expect, test, vi } from "vitest";

import { processTranscriptFile } from "./transcript";
import { createEmptyLoadedSessionData } from "./types";

describe("processTranscriptFile", () => {
  test("parses valid transcript JSON and populates result", () => {
    const result = createEmptyLoadedSessionData();
    const content = JSON.stringify({
      transcripts: [
        {
          id: "transcript-1",
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          session_id: "session-1",
          started_at: 0,
          words: [{ id: "w1", text: "hello" }],
          speaker_hints: [{ id: "sh1", speaker: "Speaker 1" }],
        },
      ],
    });

    processTranscriptFile("/path/to/transcript.json", content, result);

    expect(result.transcripts["transcript-1"]).toEqual({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      session_id: "session-1",
      started_at: 0,
      memo_md: "",
      words: JSON.stringify([{ id: "w1", text: "hello" }]),
      speaker_hints: JSON.stringify([{ id: "sh1", speaker: "Speaker 1" }]),
    });
  });

  test("handles multiple transcripts in single file", () => {
    const result = createEmptyLoadedSessionData();
    const content = JSON.stringify({
      transcripts: [
        {
          id: "transcript-1",
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          session_id: "session-1",
          started_at: 0,
          words: [],
          speaker_hints: [],
        },
        {
          id: "transcript-2",
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          session_id: "session-1",
          started_at: 100,
          words: [],
          speaker_hints: [],
        },
      ],
    });

    processTranscriptFile("/path/to/transcript.json", content, result);

    expect(Object.keys(result.transcripts)).toHaveLength(2);
    expect(result.transcripts["transcript-1"]).toBeDefined();
    expect(result.transcripts["transcript-2"]).toBeDefined();
  });

  test("handles parse errors gracefully", () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const result = createEmptyLoadedSessionData();

    processTranscriptFile("/path/to/transcript.json", "invalid json", result);

    expect(Object.keys(result.transcripts)).toHaveLength(0);
    expect(consoleSpy).toHaveBeenCalled();
    consoleSpy.mockRestore();
  });
});
