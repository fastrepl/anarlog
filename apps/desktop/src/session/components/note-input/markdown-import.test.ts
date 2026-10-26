import { describe, expect, it } from "vitest";

import {
  isFileDrag,
  isMarkdownFile,
  markdownTitleFromFileName,
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

describe("isFileDrag", () => {
  it("detects file drags from types, items, or files", () => {
    expect(isFileDrag({ types: ["Files"] } as unknown as DataTransfer)).toBe(
      true,
    );
    expect(
      isFileDrag({ items: [{ kind: "file" }] } as unknown as DataTransfer),
    ).toBe(true);
    expect(
      isFileDrag({
        files: [{ name: "a.md" }],
        items: [],
      } as unknown as DataTransfer),
    ).toBe(true);
  });

  it("rejects text drags", () => {
    expect(
      isFileDrag({ items: [{ kind: "string" }] } as unknown as DataTransfer),
    ).toBe(false);
    expect(
      isFileDrag({ types: ["text/plain"] } as unknown as DataTransfer),
    ).toBe(false);
  });
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
