import { beforeEach, describe, expect, test, vi } from "vitest";

import { initSessionOps, moveSessionToFolder, renameFolder } from "./ops";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

const fsSyncMocks = vi.hoisted(() => ({
  moveSession: vi.fn(),
  renameFolder: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({ commands: fsSyncMocks }));

describe("sessionOps", () => {
  const store = createTestMainStore();

  beforeEach(() => {
    store.delTables();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      folder_id: "work",
      event_json: "",
      title: "Test session",
      raw_md: "",
    });

    vi.clearAllMocks();
    initSessionOps({ store });
  });

  test("moveSessionToFolder applies Rust-returned folder id on success", async () => {
    fsSyncMocks.moveSession.mockResolvedValue({
      status: "ok",
      data: {
        sessionId: "session-1",
        folderId: "work/project-a",
      },
    });

    const result = await moveSessionToFolder("session-1", "work/project-a/");

    expect(result).toEqual({ status: "ok" });
    expect(fsSyncMocks.moveSession).toHaveBeenCalledWith(
      "session-1",
      "work",
      "work/project-a/",
    );
    expect(store.getCell("sessions", "session-1", "folder_id")).toBe(
      "work/project-a",
    );
  });

  test("moveSessionToFolder leaves store unchanged on error", async () => {
    fsSyncMocks.moveSession.mockResolvedValue({
      status: "error",
      error: "session_source_missing",
    });

    const result = await moveSessionToFolder("session-1", "archive");

    expect(result).toEqual({
      status: "error",
      error: "session_source_missing",
    });
    expect(store.getCell("sessions", "session-1", "folder_id")).toBe("work");
  });

  test("renameFolder applies returned updates transactionally", async () => {
    store.setRow("sessions", "session-2", {
      user_id: "user-1",
      created_at: "2024-01-02T00:00:00Z",
      folder_id: "work/nested",
      event_json: "",
      title: "Nested session",
      raw_md: "",
    });

    fsSyncMocks.renameFolder.mockResolvedValue({
      status: "ok",
      data: {
        updates: [
          { sessionId: "session-1", folderId: "archive" },
          { sessionId: "session-2", folderId: "archive/nested" },
        ],
      },
    });

    const result = await renameFolder("work", "archive");

    expect(result).toEqual({ status: "ok" });
    expect(store.getCell("sessions", "session-1", "folder_id")).toBe("archive");
    expect(store.getCell("sessions", "session-2", "folder_id")).toBe(
      "archive/nested",
    );
  });

  test("renameFolder leaves store unchanged on error", async () => {
    fsSyncMocks.renameFolder.mockResolvedValue({
      status: "error",
      error: "folder_target_exists",
    });

    const result = await renameFolder("work", "archive");

    expect(result).toEqual({
      status: "error",
      error: "folder_target_exists",
    });
    expect(store.getCell("sessions", "session-1", "folder_id")).toBe("work");
  });
});
