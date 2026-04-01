import { beforeEach, describe, expect, test, vi } from "vitest";

import { processMdFile } from "./note";
import { createEmptyLoadedSessionData, type LoadedSessionData } from "./types";

const fsSyncMocks = vi.hoisted(() => ({
  deserialize: vi.fn(),
}));

const tiptapMocks = vi.hoisted(() => ({
  md2json: vi.fn().mockReturnValue({ type: "doc", content: [] }),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({ commands: fsSyncMocks }));
vi.mock("@hypr/tiptap/shared", () => tiptapMocks);

describe("processMdFile", () => {
  let result: LoadedSessionData;

  beforeEach(() => {
    result = createEmptyLoadedSessionData();
    vi.clearAllMocks();
  });

  test("processes memo file and sets raw_md on existing session", async () => {
    result.sessions["session-1"] = {
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Session",
      folder_id: "/sessions",
      event_json: "",
      raw_md: "",
    };

    fsSyncMocks.deserialize.mockResolvedValue({
      status: "ok",
      data: {
        frontmatter: {
          id: "note-1",
          session_id: "session-1",
        },
        content: "# Hello",
      },
    });

    await processMdFile("/path/to/_memo.md", "---\n---\n# Hello", result);

    expect(result.sessions["session-1"].raw_md).toBe(
      JSON.stringify({ type: "doc", content: [] }),
    );
  });

  test("processes enhanced note file and creates entry", async () => {
    fsSyncMocks.deserialize.mockResolvedValue({
      status: "ok",
      data: {
        frontmatter: {
          id: "enhanced-1",
          session_id: "session-1",
          template_id: "template-1",
          position: 2,
          title: "My Note",
        },
        content: "# Content",
      },
    });

    await processMdFile("/path/to/_summary.md", "---\n---\n# Content", result);

    expect(result.enhanced_notes["enhanced-1"]).toMatchObject({
      session_id: "session-1",
      content: JSON.stringify({ type: "doc", content: [] }),
      template_id: "template-1",
      position: 2,
      title: "My Note",
    });
  });

  test("handles deserialize errors gracefully", async () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    fsSyncMocks.deserialize.mockResolvedValue({
      status: "error",
      error: "Parse error",
    });

    await processMdFile("/path/to/note.md", "invalid", result);

    expect(Object.keys(result.enhanced_notes)).toHaveLength(0);
    expect(consoleSpy).toHaveBeenCalled();
    consoleSpy.mockRestore();
  });

  test("skips entries with missing required frontmatter fields", async () => {
    fsSyncMocks.deserialize.mockResolvedValue({
      status: "ok",
      data: {
        frontmatter: {
          id: "note-1",
        },
        content: "# Hello",
      },
    });

    await processMdFile("/path/to/note.md", "---\n---\n# Hello", result);

    expect(Object.keys(result.enhanced_notes)).toHaveLength(0);
  });
});
