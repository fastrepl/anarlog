import { describe, expect, it } from "vitest";

import {
  collectFolderPaths,
  folderBreadcrumbLabel,
  folderPathSegments,
  normalizeFolderPath,
} from "./folders";

describe("folder paths", () => {
  it("normalizes nested paths and empty values", () => {
    expect(normalizeFolderPath("")).toBe("");
    expect(normalizeFolderPath("   ")).toBe("");
    expect(normalizeFolderPath("work")).toBe("work");
    expect(normalizeFolderPath("work/meetings/")).toBe("work/meetings");
    expect(normalizeFolderPath(String.raw`work\meetings`)).toBe(
      "work/meetings",
    );
  });

  it("rejects traversal and absolute paths", () => {
    expect(normalizeFolderPath("/work")).toBeNull();
    expect(normalizeFolderPath("work//meetings")).toBeNull();
    expect(normalizeFolderPath("../work")).toBeNull();
    expect(normalizeFolderPath("work/../meetings")).toBeNull();
    expect(normalizeFolderPath("./work")).toBeNull();
  });

  it("expands ancestor folder paths from stored session paths", () => {
    expect(collectFolderPaths(["work/meetings", "personal", ""])).toEqual([
      "personal",
      "work",
      "work/meetings",
    ]);
  });

  it("formats breadcrumb labels from path segments", () => {
    expect(folderPathSegments("work/meetings")).toEqual(["work", "meetings"]);
    expect(folderBreadcrumbLabel("work/meetings")).toBe("work / meetings");
    expect(folderBreadcrumbLabel("")).toBe("");
  });
});
