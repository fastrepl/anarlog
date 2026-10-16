import { beforeEach, describe, expect, it, vi } from "vitest";

import { openFoldersFromNavigation } from "./folders-navigation";

import { useFolderSelection } from "~/folders/selection";

describe("openFoldersFromNavigation", () => {
  beforeEach(() => {
    useFolderSelection.setState({ selectedPath: null });
  });

  it("selects the note folder when the main window receives see all folders", () => {
    const openNew = vi.fn();

    const handled = openFoldersFromNavigation(
      { path: "/app/folders", search: { path: "work" } },
      openNew,
    );

    expect(handled).toBe(true);
    expect(useFolderSelection.getState().selectedPath).toBe("work");
    expect(openNew).toHaveBeenCalledWith({ type: "folders" });
  });
});
