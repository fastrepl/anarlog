import { describe, expect, test, vi } from "vitest";

import { buildNoteSaveOps } from "./note";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

vi.mock("@tauri-apps/api/path", () => ({
  sep: () => "/",
}));

describe("buildNoteSaveOps", () => {
  test("persists empty enhanced notes so standalone windows can render the tab", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "work",
      event_json: "",
      raw_md: "",
    });
    store.setRow("enhanced_notes", "summary-1", {
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
      "/data",
      new Set(["session-1"]),
      { deleteEmptyMemos: false },
    );

    expect(ops).toEqual([
      {
        type: "write-document-batch",
        items: [
          [
            {
              frontmatter: {
                id: "summary-1",
                session_id: "session-1",
                position: 1,
                title: "Summary",
              },
              content: "",
            },
            "/data/sessions/work/session-1/_summary.md",
          ],
        ],
      },
    ]);
  });

  test("does not delete empty memos when folder-only changes are saved", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "work",
      event_json: "",
      raw_md: "",
    });

    const ops = buildNoteSaveOps(
      store,
      store.getTables(),
      "/data",
      new Set(["session-1"]),
      { deleteEmptyMemos: false },
    );

    expect(ops).toEqual([]);
  });

  test("deletes empty memos when note content is cleared", () => {
    const store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "work",
      event_json: "",
      raw_md: "",
    });

    const ops = buildNoteSaveOps(
      store,
      store.getTables(),
      "/data",
      new Set(["session-1"]),
      { deleteEmptyMemos: true },
    );

    expect(ops).toEqual([
      {
        type: "delete",
        paths: ["/data/sessions/work/session-1/_memo.md"],
      },
    ]);
  });
});
