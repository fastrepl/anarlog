import { describe, expect, it } from "vitest";

import { parseSessionTagNames, resolveSidebarItemMeta } from "./item-fields";

describe("parseSessionTagNames", () => {
  it("trims, dedupes, and sorts JSON tag names", () => {
    expect(
      parseSessionTagNames(JSON.stringify(["prep", " launch", "prep"])),
    ).toEqual(["launch", "prep"]);
  });

  it("keeps commas that belong to a tag name", () => {
    expect(
      parseSessionTagNames(JSON.stringify(["launch, prep", "design"])),
    ).toEqual(["design", "launch, prep"]);
  });

  it("returns no tags for empty or invalid input", () => {
    expect(parseSessionTagNames(null)).toEqual([]);
    expect(parseSessionTagNames("")).toEqual([]);
    expect(parseSessionTagNames("not-json")).toEqual([]);
    expect(parseSessionTagNames(JSON.stringify({ name: "launch" }))).toEqual(
      [],
    );
  });
});

describe("resolveSidebarItemMeta", () => {
  it.each([
    ["shows the folder by default", true, false, "date", "CS 101/week-3", []],
    [
      "hides the folder when grouped by folder",
      true,
      true,
      "folder",
      "",
      ["launch"],
    ],
    ["omits fields the user turned off", false, false, "date", "", []],
  ] as const)("%s", (_, showFolder, showTags, groupBy, folder, tags) => {
    expect(
      resolveSidebarItemMeta({
        folderId: "CS 101/week-3",
        tags: ["launch"],
        showFolder,
        showTags,
        groupBy,
      }),
    ).toEqual({ folder, tags });
  });
});
