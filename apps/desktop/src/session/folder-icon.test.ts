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

  it("keeps a custom icon and unwraps nested JSON", () => {
    const icon = { type: "icon" as const, value: "target", color: "#5b67d8" };
    expect(normalizeFolderIcon(icon)).toEqual(icon);
    expect(normalizeFolderIcon(JSON.stringify(JSON.stringify(icon)))).toEqual(
      icon,
    );
  });

  it("falls back to the folder default for invalid values", () => {
    expect(normalizeFolderIcon("{")).toEqual(DEFAULT_FOLDER_ICON);
    expect(normalizeFolderIcon({ type: "icon" })).toEqual(DEFAULT_FOLDER_ICON);
  });
});
