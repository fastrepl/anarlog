import { describe, expect, it } from "vitest";

import { DEFAULT_FOLDER_ICON, normalizeFolderIcon } from "./folder-icon";

describe("normalizeFolderIcon", () => {
  it("uses the folder default when the value is missing", () => {
    expect(normalizeFolderIcon(null)).toEqual(DEFAULT_FOLDER_ICON);
    expect(normalizeFolderIcon("")).toEqual(DEFAULT_FOLDER_ICON);
  });

  it("keeps a stored folder icon", () => {
    expect(
      normalizeFolderIcon('{"type":"icon","value":"folder","color":"#9ca3af"}'),
    ).toEqual(DEFAULT_FOLDER_ICON);
  });
});
