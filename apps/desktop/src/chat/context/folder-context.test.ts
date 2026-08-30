import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadSessionSummariesByFolder: vi.fn(),
}));

vi.mock("~/session/queries", () => ({
  loadSessionSummariesByFolder: mocks.loadSessionSummariesByFolder,
}));

import { renderFolderContext } from "./folder-context";

describe("folder chat context", () => {
  beforeEach(() => {
    mocks.loadSessionSummariesByFolder.mockReset();
  });

  it("renders a folder index with session ids", async () => {
    mocks.loadSessionSummariesByFolder.mockResolvedValue([
      {
        id: "session-1",
        title: "Lecture 1",
        created_at: "2026-08-01T09:00:00.000Z",
      },
      {
        id: "session-2",
        title: "",
        created_at: "2026-08-08T09:00:00.000Z",
      },
    ]);

    await expect(renderFolderContext("CS 101")).resolves.toEqual(
      [
        "Folder context: CS 101",
        "Answer from notes in this folder. Use get_meeting or search_meetings with the listed IDs when you need full notes or transcripts.",
        "",
        "- Lecture 1 (2026-08-01T09:00:00.000Z) [session-1]",
        "- Untitled (2026-08-08T09:00:00.000Z) [session-2]",
      ].join("\n"),
    );
  });

  it("labels the unfiled view and empty folders", async () => {
    mocks.loadSessionSummariesByFolder.mockResolvedValue([]);

    await expect(renderFolderContext("")).resolves.toContain(
      "Folder context: No folder",
    );
    await expect(renderFolderContext("")).resolves.toContain(
      "This folder has no notes yet.",
    );
  });
});
