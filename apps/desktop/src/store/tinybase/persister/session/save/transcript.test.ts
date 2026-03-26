import { describe, expect, test, vi } from "vitest";

import { buildTranscriptSaveOps } from "./transcript";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

vi.mock("@tauri-apps/api/path", () => ({
  sep: () => "/",
}));

describe("buildTranscriptSaveOps", () => {
  const dataDir = "/data";

  test("writes only transcripts with words", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });
    store.setRow("transcripts", "transcript-empty", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      session_id: "session-1",
      started_at: 0,
      ended_at: 10,
      memo_md: "",
      words: JSON.stringify([]),
      speaker_hints: JSON.stringify([]),
    });
    store.setRow("transcripts", "transcript-ready", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:01Z",
      session_id: "session-1",
      started_at: 10,
      ended_at: 20,
      memo_md: "",
      words: JSON.stringify([{ id: "w1", text: "hello" }]),
      speaker_hints: JSON.stringify([]),
    });

    const ops = buildTranscriptSaveOps(store.getTables(), dataDir);

    expect(ops).toEqual([
      {
        type: "write-json",
        path: "/data/sessions/session-1/transcript.json",
        content: {
          transcripts: [
            {
              id: "transcript-ready",
              user_id: "user-1",
              created_at: "2024-01-01T00:00:01Z",
              session_id: "session-1",
              started_at: 10,
              ended_at: 20,
              memo_md: "",
              words: [{ id: "w1", text: "hello" }],
              speaker_hints: [],
            },
          ],
        },
      },
    ]);
  });

  test("deletes transcript.json when a changed session has no usable transcript content", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });
    store.setRow("transcripts", "transcript-empty", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      session_id: "session-1",
      started_at: 0,
      ended_at: 10,
      memo_md: "",
      words: JSON.stringify([]),
      speaker_hints: JSON.stringify([]),
    });

    const ops = buildTranscriptSaveOps(
      store.getTables(),
      dataDir,
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
