import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadFolderMaterials: vi.fn(),
  loadSessionSummariesByFolder: vi.fn(),
}));

vi.mock("~/session/folder-attachments", () => ({
  loadFolderMaterials: mocks.loadFolderMaterials,
}));

vi.mock("~/session/queries", () => ({
  loadSessionSummariesByFolder: mocks.loadSessionSummariesByFolder,
}));

import { renderFolderContext } from "./folder-context";

describe("folder chat context", () => {
  beforeEach(() => {
    mocks.loadSessionSummariesByFolder.mockReset();
    mocks.loadFolderMaterials.mockReset();
    mocks.loadFolderMaterials.mockResolvedValue([]);
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
        "Answer from notes in this folder. Use get_meeting or search_meetings with the listed IDs when you need full notes or transcripts. Use read_folder_material with a listed material ID to read a syllabus or other folder file.",
        "",
        "- Lecture 1 (2026-08-01T09:00:00.000Z) [session-1]",
        "- Untitled (2026-08-08T09:00:00.000Z) [session-2]",
      ].join("\n"),
    );
    expect(mocks.loadFolderMaterials).toHaveBeenCalledWith("CS 101");
  });

  it("lists folder materials before notes", async () => {
    mocks.loadSessionSummariesByFolder.mockResolvedValue([
      {
        id: "session-1",
        title: "Lecture 1",
        created_at: "2026-08-01T09:00:00.000Z",
      },
    ]);
    mocks.loadFolderMaterials.mockResolvedValue([
      {
        id: "mat-1",
        filename: "syllabus.pdf",
        contentType: "application/pdf",
        sizeBytes: 2048,
        relativePath: "materials/syllabus.pdf",
      },
    ]);

    await expect(renderFolderContext("CS 101")).resolves.toContain(
      "- syllabus.pdf [mat-1]",
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
