import { describe, expect, it } from "vitest";

import {
  formatSidebarItemMetaLine,
  parseSessionTagNames,
  resolveSidebarItemMeta,
} from "./item-fields";

describe("parseSessionTagNames", () => {
  it("splits, trims, dedupes, and sorts tag names", () => {
    expect(parseSessionTagNames("prep, launch,prep, ")).toEqual([
      "launch",
      "prep",
    ]);
  });

  it("returns no tags for empty input", () => {
    expect(parseSessionTagNames(null)).toEqual([]);
    expect(parseSessionTagNames("")).toEqual([]);
  });
});

describe("resolveSidebarItemMeta", () => {
  it("shows the folder above the title by default", () => {
    expect(
      resolveSidebarItemMeta({
        folderId: "CS 101/week-3",
        tags: ["launch"],
        showFolder: true,
        showTags: false,
        groupBy: "date",
      }),
    ).toEqual({ folder: "CS 101/week-3", tags: [] });
  });

  it("hides the folder when notes are already grouped by folder", () => {
    expect(
      resolveSidebarItemMeta({
        folderId: "work",
        tags: ["launch"],
        showFolder: true,
        showTags: true,
        groupBy: "folder",
      }),
    ).toEqual({ folder: "", tags: ["launch"] });
  });

  it("omits fields the user turned off", () => {
    expect(
      resolveSidebarItemMeta({
        folderId: "work",
        tags: ["launch"],
        showFolder: false,
        showTags: false,
        groupBy: "date",
      }),
    ).toEqual({ folder: "", tags: [] });
  });
});

describe("formatSidebarItemMetaLine", () => {
  it("joins folder and tags on one line", () => {
    expect(formatSidebarItemMetaLine("work", ["launch", "prep"])).toBe(
      "work · #launch #prep",
    );
  });
});
