import { beforeEach, describe, expect, test, vi } from "vitest";

import { deleteSessionCascade, finalizeSessionDeletion } from "./deleteSession";

import { createTestMainStore } from "~/store/tinybase/persister/testing/mocks";

const fsSyncMocks = vi.hoisted(() => ({
  audioDelete: vi.fn(),
  deleteSessionFolder: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({ commands: fsSyncMocks }));

describe("deleteSessionCascade", () => {
  let store: ReturnType<typeof createTestMainStore>;

  beforeEach(() => {
    store = createTestMainStore();
    store.setRow("sessions", "session-1", {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      folder_id: "",
      event_json: "",
      title: "Meeting notes",
      raw_md: "",
    });

    fsSyncMocks.deleteSessionFolder.mockResolvedValue({
      status: "ok",
      data: null,
    });
    vi.clearAllMocks();
  });

  test("deletes the persisted session folder by default", () => {
    deleteSessionCascade(store, undefined, "session-1");

    expect(store.getRow("sessions", "session-1")).toEqual({});
    expect(fsSyncMocks.deleteSessionFolder).toHaveBeenCalledWith("session-1");
    expect(fsSyncMocks.audioDelete).not.toHaveBeenCalled();
  });

  test("defers filesystem deletion while undo is pending", () => {
    deleteSessionCascade(store, undefined, "session-1", {
      deferFilesystemDelete: true,
    });

    expect(store.getRow("sessions", "session-1")).toEqual({});
    expect(fsSyncMocks.deleteSessionFolder).not.toHaveBeenCalled();
  });

  test("finalizes deletion by removing the session folder", async () => {
    await finalizeSessionDeletion("session-1");

    expect(fsSyncMocks.deleteSessionFolder).toHaveBeenCalledWith("session-1");
  });
});
