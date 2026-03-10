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
          memo_md: "memo",
          words: [
            {
              id: "w1",
              text: "hello",
              start_ms: 0,
              end_ms: 100,
              channel: 0,
              speaker: "Speaker 1",
              metadata: { confidence: 0.98 },
            },
          ],
          speaker_hints: [
            {
              id: "sh1",
              word_id: "w1",
              type: "speaker_label",
              value: { label: "Speaker 1" },
            },
          ],
        },
      ],
    });

    processTranscriptFile("/path/to/transcript.json", content, result);

    expect(result.transcripts["transcript-1"]).toEqual({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      session_id: "session-1",
      started_at: 0,
      memo_md: "memo",
      words: JSON.stringify([
        {
          id: "w1",
          text: "hello",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
          speaker: "Speaker 1",
          metadata: { confidence: 0.98 },
        },
      ]),
      speaker_hints: JSON.stringify([
        {
          id: "sh1",
          word_id: "w1",
          type: "speaker_label",
          value: { label: "Speaker 1" },
        },
      ]),
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

  test("preserves real filesystem hint payload strings with omitted ended_at", () => {
    const result = createEmptyLoadedSessionData();
    const content = JSON.stringify({
      transcripts: [
        {
          id: "transcript-1",
          user_id: "00000000-0000-0000-0000-000000000000",
          created_at: "2026-02-24T09:36:44.069Z",
          session_id: "session-1",
          started_at: 1771925804069,
          memo_md: "",
          words: [
            {
              channel: 2,
              end_ms: 7189,
              id: "word-1",
              start_ms: 7129,
              text: " Ne",
            },
          ],
          speaker_hints: [
            {
              id: "hint-1",
              type: "provider_speaker_index",
              value: '{"provider":"deepgram","channel":2,"speaker_index":0}',
              word_id: "word-1",
            },
          ],
        },
      ],
    });

    processTranscriptFile("/path/to/transcript.json", content, result);

    expect(result.transcripts["transcript-1"]).toEqual({
      user_id: "00000000-0000-0000-0000-000000000000",
      created_at: "2026-02-24T09:36:44.069Z",
      session_id: "session-1",
      started_at: 1771925804069,
      ended_at: undefined,
      memo_md: "",
      words: JSON.stringify([
        {
          channel: 2,
          end_ms: 7189,
          id: "word-1",
          start_ms: 7129,
          text: " Ne",
        },
      ]),
      speaker_hints: JSON.stringify([
        {
          id: "hint-1",
          type: "provider_speaker_index",
          value: '{"provider":"deepgram","channel":2,"speaker_index":0}',
          word_id: "word-1",
        },
      ]),
    });
  });

  test("normalizes legacy null and omitted transcript fields", () => {
    const result = createEmptyLoadedSessionData();
    const content = JSON.stringify({
      transcripts: [
        {
          id: "transcript-1",
          user_id: null,
          created_at: null,
          session_id: "session-1",
          started_at: null,
          ended_at: null,
          memo_md: null,
          words: [
            {
              text: "hello",
              start_ms: 0,
              end_ms: 100,
              channel: 0,
              speaker: null,
              metadata: null,
            },
          ],
          speaker_hints: null,
        },
        {
          id: "transcript-2",
          session_id: "session-1",
          words: [
            {
              text: "world",
              start_ms: 100,
              end_ms: 200,
              channel: 0,
            },
          ],
          speaker_hints: [
            {
              word_id: "word-1",
              type: "speaker_label",
            },
          ],
        },
      ],
    });

    processTranscriptFile("/path/to/transcript.json", content, result);

    expect(result.transcripts["transcript-1"]).toEqual({
      user_id: "",
      created_at: "",
      session_id: "session-1",
      started_at: 0,
      ended_at: undefined,
      memo_md: "",
      words: JSON.stringify([
        {
          text: "hello",
          start_ms: 0,
          end_ms: 100,
          channel: 0,
          speaker: null,
          metadata: null,
        },
      ]),
      speaker_hints: JSON.stringify([]),
    });
    expect(result.transcripts["transcript-2"]).toEqual({
      user_id: "",
      created_at: "",
      session_id: "session-1",
      started_at: 0,
      ended_at: undefined,
      memo_md: "",
      words: JSON.stringify([
        {
          text: "world",
          start_ms: 100,
          end_ms: 200,
          channel: 0,
        },
      ]),
      speaker_hints: JSON.stringify([
        {
          word_id: "word-1",
          type: "speaker_label",
        },
      ]),
    });
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
