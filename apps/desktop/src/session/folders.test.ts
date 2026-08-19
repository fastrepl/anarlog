import { describe, expect, it } from "vitest";

import {
  collectFolderPaths,
  folderDisplayName,
  normalizeFolderPath,
} from "./folders";

describe("folder paths", () => {
  it("normalizes a single folder name and empty values", () => {
    expect(normalizeFolderPath("")).toBe("");
    expect(normalizeFolderPath("   ")).toBe("");
    expect(normalizeFolderPath("work")).toBe("work");
    expect(normalizeFolderPath("  meetings  ")).toBe("meetings");
  });

  it("rejects nested, traversal, and absolute paths", () => {
    expect(normalizeFolderPath("/work")).toBeNull();
    expect(normalizeFolderPath("work/meetings")).toBeNull();
    expect(normalizeFolderPath("work/meetings/")).toBeNull();
    expect(normalizeFolderPath(String.raw`work\meetings`)).toBeNull();
    expect(normalizeFolderPath("work//meetings")).toBeNull();
    expect(normalizeFolderPath("../work")).toBeNull();
    expect(normalizeFolderPath("work/../meetings")).toBeNull();
    expect(normalizeFolderPath("./work")).toBeNull();
    expect(normalizeFolderPath(".")).toBeNull();
    expect(normalizeFolderPath("..")).toBeNull();
  });

  it("lists unique top-level folder names", () => {
    expect(
      collectFolderPaths(["work/meetings", "personal", "work", ""]),
    ).toEqual(["personal", "work"]);
  });

  it("displays the top-level name from stored paths", () => {
    expect(folderDisplayName("work")).toBe("work");
    expect(folderDisplayName("work/meetings")).toBe("work");
    expect(folderDisplayName("")).toBe("");
    expect(folderDisplayName(null)).toBe("");
  });
});
