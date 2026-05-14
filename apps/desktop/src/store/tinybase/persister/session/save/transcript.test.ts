import { describe, expect, test, vi } from "vitest";

import { buildTranscriptSaveOps } from "./transcript";

import type { TablesContent } from "~/store/tinybase/persister/shared";

vi.mock("@tauri-apps/api/path", () => ({
  sep: () => "/",
}));

describe("buildTranscriptSaveOps", () => {
  const dataDir = "/data";

  test("only parses transcripts for changed sessions", () => {
    const tables: TablesContent = {
      sessions: {
        "session-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          title: "Changed Session",
          folder_id: "",
          event_json: "",
          raw_md: "",
        },
        "session-2": {
          user_id: "user-1",
          created_at: "2024-01-02T00:00:00Z",
          title: "Unchanged Session",
          folder_id: "",
          event_json: "",
          raw_md: "",
        },
      },
      transcripts: {
        "transcript-1": {
          session_id: "session-1",
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          started_at: 100,
          words: "[]",
          speaker_hints: "[]",
          memo_md: "",
        },
        "transcript-2": {
          session_id: "session-2",
          user_id: "user-1",
          created_at: "2024-01-02T00:00:00Z",
          started_at: 200,
          words: "{invalid json",
          speaker_hints: "[]",
          memo_md: "",
        },
      },
    };

    const ops = buildTranscriptSaveOps(tables, dataDir, new Set(["session-1"]));

    expect(ops).toHaveLength(1);
    expect(ops[0]).toMatchObject({
      type: "write-json",
      path: "/data/sessions/session-1/transcript.json",
    });
  });
});
