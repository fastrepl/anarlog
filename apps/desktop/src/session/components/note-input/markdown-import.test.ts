import { describe, expect, it } from "vitest";

import {
  isMarkdownFile,
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
