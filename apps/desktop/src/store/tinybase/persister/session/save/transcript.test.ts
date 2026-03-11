import { describe, expect, test } from "vitest";

import { buildTranscriptSaveOps } from "./transcript";

describe("buildTranscriptSaveOps", () => {
  test("writes transcript.json when the session meets the threshold", () => {
    const ops = buildTranscriptSaveOps(
      {
        sessions: {
          "session-1": {
            folder_id: "",
          },
        },
        transcripts: {
          "transcript-1": {
            session_id: "session-1",
            user_id: "user-1",
            created_at: "2026-03-11T00:00:00.000Z",
            started_at: 1,
            words: JSON.stringify(
              Array.from({ length: 5 }, (_, index) => ({
                id: `word-${index}`,
                text: `word-${index}`,
              })),
            ),
            speaker_hints: "[]",
            memo_md: "",
          },
        },
      },
      "/data",
    );

    expect(ops).toEqual([
      {
        type: "write-json",
        path: "/data/sessions/session-1/transcript.json",
        content: {
          transcripts: [
            expect.objectContaining({
              id: "transcript-1",
              session_id: "session-1",
            }),
          ],
        },
      },
    ]);
  });

  test("deletes transcript.json when the session stays below the threshold", () => {
    const ops = buildTranscriptSaveOps(
      {
        sessions: {
          "session-1": {
            folder_id: "",
          },
        },
        transcripts: {
          "transcript-1": {
            session_id: "session-1",
            user_id: "user-1",
            created_at: "2026-03-11T00:00:00.000Z",
            started_at: 1,
            words: JSON.stringify([
              { id: "word-1", text: "hello" },
              { id: "word-2", text: "world" },
            ]),
            speaker_hints: "[]",
            memo_md: "",
          },
        },
      },
      "/data",
    );

    expect(ops).toEqual([
      {
        type: "delete",
        paths: ["/data/sessions/session-1/transcript.json"],
      },
    ]);
  });

  test("deletes transcript.json for changed sessions with no transcript rows left", () => {
    const ops = buildTranscriptSaveOps(
      {
        sessions: {
          "session-1": {
            folder_id: "",
          },
        },
        transcripts: {},
      },
      "/data",
      new Set(["session-1"]),
    );

    expect(ops).toEqual([
      {
        type: "delete",
        paths: ["/data/sessions/session-1/transcript.json"],
      },
    ]);
  });
});
