import { describe, expect, it } from "vitest";

import {
  isFileDrag,
  isMarkdownFile,
  markdownDragOverlayForPaths,
  markdownTitleFromFileName,
  matchMarkdownDrag,
  matchMarkdownDrop,
} from "./markdown-import";

describe("isMarkdownFile", () => {
  it.each(["notes.md", "NOTES.MD", "notes.markdown", "notes.mdown"])(
    "accepts %s",
    (name) => {
      expect(isMarkdownFile({ name })).toBe(true);
    },
  );

  it.each(["notes.txt", "notes.mdx", "notes.mp3", "md", "notes.md.zip"])(
    "rejects %s",
    (name) => {
      expect(isMarkdownFile({ name })).toBe(false);
    },
  );
});

describe("markdownTitleFromFileName", () => {
  it.each([
    ["notes.md", "notes"],
    ["my.notes.md", "my.notes"],
    ["noext", "noext"],
    [".md", ".md"],
  ])("maps %s to %s", (fileName, title) => {
    expect(markdownTitleFromFileName(fileName)).toBe(title);
  });
});

describe("matchMarkdownDrag", () => {
  it("matches a single markdown file with its name", () => {
    expect(
      matchMarkdownDrag(dataTransferWithItems([fileItem("notes.md")])),
    ).toEqual({
      active: true,
      subtitle: "Will be added as a new note",
      title: 'Import "notes.md"',
    });
  });

  it("matches multiple markdown files with a count", () => {
    expect(
      matchMarkdownDrag(
        dataTransferWithItems([fileItem("a.md"), fileItem("b.md")]),
      ),
    ).toEqual({
      active: true,
      subtitle: "Will be added as 2 new notes",
      title: "Import 2 files",
    });
  });

  it.each([
    ["mixed markdown and text", [fileItem("a.md"), fileItem("b.txt")]],
    ["text only", [fileItem("b.txt")]],
    ["non-file drag", [{ kind: "string" }]],
  ])("stays inactive for %s", (_label, items) => {
    expect(matchMarkdownDrag(dataTransferWithItems(items))).toEqual({
      active: false,
    });
  });

  it("falls back to files when items are empty", () => {
    expect(
      matchMarkdownDrag(
        dataTransferWithItems([], [{ name: "notes.md" } as File]),
      ),
    ).toEqual({
      active: true,
      subtitle: "Will be added as a new note",
      title: 'Import "notes.md"',
    });
  });
});

describe("isFileDrag", () => {
  it("detects file drags from types, items, or files", () => {
    expect(isFileDrag({ types: ["Files"] } as unknown as DataTransfer)).toBe(
      true,
    );
    expect(isFileDrag(dataTransferWithItems([{ kind: "file" }]))).toBe(true);
    expect(
      isFileDrag(dataTransferWithItems([], [{ name: "a.md" } as File])),
    ).toBe(true);
  });

  it("rejects text drags", () => {
    expect(isFileDrag(dataTransferWithItems([{ kind: "string" }]))).toBe(false);
    expect(
      isFileDrag({ types: ["text/plain"] } as unknown as DataTransfer),
    ).toBe(false);
  });
});

describe("markdownDragOverlayForPaths", () => {
  it("shows the file name for a single markdown hover path", () => {
    expect(markdownDragOverlayForPaths(["/Users/tony/notes.md"])).toEqual({
      active: true,
      subtitle: "Will be added as a new note",
      title: 'Import "notes.md"',
    });
  });

  it("shows a count for multiple markdown hover paths", () => {
    expect(
      markdownDragOverlayForPaths(["C:\\a.md", "/tmp/b.markdown"]),
    ).toEqual({
      active: true,
      subtitle: "Will be added as 2 new notes",
      title: "Import 2 files",
    });
  });

  it.each([[[]], [["/tmp/a.md", "/tmp/b.txt"]], [["/tmp/photo.png"]]])(
    "stays inactive for %s",
    (paths) => {
      expect(markdownDragOverlayForPaths(paths)).toEqual({ active: false });
    },
  );
});

describe("matchMarkdownDrop", () => {
  it("accepts only all-markdown drops", () => {
    const files = [{ name: "a.md" } as File, { name: "b.md" } as File];
    expect(matchMarkdownDrop(files)).toBe(files);
    expect(matchMarkdownDrop([])).toBeNull();
    expect(
      matchMarkdownDrop([{ name: "a.md" } as File, { name: "b.txt" } as File]),
    ).toBeNull();
  });
});

function fileItem(name: string): DataTransferItem {
  return {
    getAsFile: () => ({ name }) as File,
    kind: "file",
  } as DataTransferItem;
}

function dataTransferWithItems(
  items: Array<Partial<DataTransferItem>>,
  files: File[] = [],
): DataTransfer {
  return { files, items } as unknown as DataTransfer;
}
