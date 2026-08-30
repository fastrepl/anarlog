import { describe, expect, it } from "vitest";

import {
  ancestorFolderPaths,
  childFolderPath,
  collectFolderPaths,
  folderDisplayName,
  folderPathMatchesFilter,
  normalizeFolderPath,
} from "./folders";

describe("folder paths", () => {
  it("normalizes a single folder name and empty values", () => {
    expect(normalizeFolderPath("")).toBe("");
    expect(normalizeFolderPath("   ")).toBe("");
    expect(normalizeFolderPath("work")).toBe("work");
    expect(normalizeFolderPath("  meetings  ")).toBe("meetings");
  });

  it("allows nested paths and normalizes separators", () => {
    expect(normalizeFolderPath("work/meetings")).toBe("work/meetings");
    expect(normalizeFolderPath("work/meetings/")).toBe("work/meetings");
    expect(normalizeFolderPath(String.raw`work\meetings`)).toBe(
      "work/meetings",
    );
  });

  it("rejects traversal, empty segments, and absolute paths", () => {
    expect(normalizeFolderPath("/work")).toBeNull();
    expect(normalizeFolderPath("work//meetings")).toBeNull();
    expect(normalizeFolderPath("../work")).toBeNull();
    expect(normalizeFolderPath("work/../meetings")).toBeNull();
    expect(normalizeFolderPath("./work")).toBeNull();
    expect(normalizeFolderPath(".")).toBeNull();
    expect(normalizeFolderPath("..")).toBeNull();
  });

  it("lists unique folder paths and their ancestors", () => {
    expect(
      collectFolderPaths(["work/meetings", "personal", "work", ""]),
    ).toEqual(["personal", "work", "work/meetings"]);
  });

  it("displays the last segment from stored paths", () => {
    expect(folderDisplayName("work")).toBe("work");
    expect(folderDisplayName("work/meetings")).toBe("meetings");
    expect(folderDisplayName("")).toBe("");
    expect(folderDisplayName(null)).toBe("");
  });

  it("matches a folder filter against the path and its children", () => {
    expect(folderPathMatchesFilter("CS 101", "CS 101")).toBe(true);
    expect(folderPathMatchesFilter("CS 101/week-3", "CS 101")).toBe(true);
    expect(folderPathMatchesFilter("CS 101 Extra", "CS 101")).toBe(false);
    expect(folderPathMatchesFilter("work", "CS 101")).toBe(false);
    expect(folderPathMatchesFilter("", "")).toBe(true);
    expect(folderPathMatchesFilter("CS 101", "")).toBe(false);
    expect(folderPathMatchesFilter("CS 101", null)).toBe(true);
  });

  it("joins a child folder under its parent", () => {
    expect(childFolderPath("CS 101", "Week 1")).toBe("CS 101/Week 1");
    expect(childFolderPath("", "Week 1")).toBeNull();
    expect(ancestorFolderPaths("CS 101/Week 1")).toEqual([
      "CS 101",
      "CS 101/Week 1",
    ]);
  });
});
