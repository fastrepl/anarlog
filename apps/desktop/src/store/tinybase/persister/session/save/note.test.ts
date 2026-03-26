import { describe, expect, test, vi } from "vitest";

import { buildNoteSaveOps } from "./note";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

vi.mock("@tauri-apps/api/path", () => ({
  sep: () => "/",
}));

describe("buildNoteSaveOps", () => {
  const dataDir = "/data";

  test("deletes the summary file when the enhanced note is empty", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "",
      event_json: "",
      raw_md: "",
    });
    store.setRow("enhanced_notes", "note-1", {
      user_id: "user-1",
      session_id: "session-1",
      content: "",
      template_id: "",
      position: 1,
      title: "Summary",
    });

    const ops = buildNoteSaveOps(
      store,
      store.getTables(),
      dataDir,
      new Set(["session-1"]),
    );

    expect(ops).toEqual([
      {
        type: "delete",
        paths: [
          "/data/sessions/session-1/_summary.md",
          "/data/sessions/session-1/_memo.md",
        ],
      },
    ]);
  });
});
