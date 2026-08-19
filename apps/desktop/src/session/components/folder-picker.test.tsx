import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FolderPicker } from "./folder-picker";

const mocks = vi.hoisted(() => ({
  folderId: "",
  folderPaths: [] as string[],
  updateSession: vi.fn(() => Promise.resolve()),
}));

vi.mock("~/session/queries", () => ({
  useFolderPaths: () => mocks.folderPaths,
  useSession: () => ({ folder_id: mocks.folderId }),
  useUpdateSession: () => mocks.updateSession,
}));

describe("FolderPicker", () => {
  beforeEach(() => {
    mocks.folderId = "";
    mocks.folderPaths = ["personal", "work", "work/meetings"];
    mocks.updateSession.mockClear();
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
    Element.prototype.scrollIntoView = vi.fn();
  });

  afterEach(() => {
    cleanup();
  });

  it("lets the user select an existing folder for the current note", async () => {
    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Select folder" }));
    fireEvent.click(screen.getByRole("option", { name: "work / meetings" }));

    expect(mocks.updateSession).toHaveBeenCalledWith({
      folder_id: "work/meetings",
    });
  });

  it("creates a folder from the search query and assigns the note", async () => {
    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Select folder" }));
    fireEvent.change(screen.getByPlaceholderText("Search or create folder"), {
      target: { value: "clients/acme" },
    });
    fireEvent.click(
      await screen.findByRole("option", { name: 'Create "clients/acme"' }),
    );

    expect(mocks.updateSession).toHaveBeenCalledWith({
      folder_id: "clients/acme",
    });
  });

  it("can remove the current note from its folder", () => {
    mocks.folderId = "work/meetings";

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(
      screen.getByRole("combobox", { name: "Folder: work / meetings" }),
    );
    fireEvent.click(screen.getByRole("option", { name: "No folder" }));

    expect(mocks.updateSession).toHaveBeenCalledWith({ folder_id: "" });
  });
});
