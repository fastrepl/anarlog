import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FolderPicker, folderPickerStyles } from "./folder-picker";

import { closestWithStyle, expectStyle } from "~/session/stylex-test";

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
    mocks.folderPaths = ["personal", "work"];
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

  it("sizes the folder icon with the other header actions", () => {
    render(<FolderPicker sessionId="session-1" />);

    const trigger = screen.getByRole("combobox", { name: "Select folder" });
    const icons = trigger.querySelectorAll("svg");

    expect(trigger.textContent).toBe("");
    expectStyle(trigger, folderPickerStyles.triggerEmpty);
    expect(icons).toHaveLength(1);
    expectStyle(icons[0], folderPickerStyles.icon);
  });

  it("shows the selected folder name without a chevron", () => {
    mocks.folderId = "work";

    render(<FolderPicker sessionId="session-1" />);

    const trigger = screen.getByRole("combobox", { name: "Folder: work" });

    expect(trigger.textContent).toBe("work");
    expect(trigger.querySelectorAll("svg")).toHaveLength(1);
  });

  it("uses the same floating chrome as note metadata", () => {
    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Select folder" }));

    const input = screen.getByPlaceholderText("Search or create folder");
    const content = input.closest("[data-radix-popper-content-wrapper] > *");
    expectStyle(content, folderPickerStyles.popover);
    expectStyle(
      closestWithStyle(input, folderPickerStyles.content),
      folderPickerStyles.content,
    );
  });

  it("lets the user select an existing folder for the current note", async () => {
    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Select folder" }));
    fireEvent.click(screen.getByRole("option", { name: "work" }));

    expect(mocks.updateSession).toHaveBeenCalledWith({
      folder_id: "work",
    });
  });

  it("creates a folder from the search query and assigns the note", async () => {
    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Select folder" }));
    fireEvent.change(screen.getByPlaceholderText("Search or create folder"), {
      target: { value: "clients" },
    });
    fireEvent.click(
      await screen.findByRole("option", { name: 'Create "clients"' }),
    );

    expect(mocks.updateSession).toHaveBeenCalledWith({
      folder_id: "clients",
    });
  });

  it("rejects nested folder names", async () => {
    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Select folder" }));
    fireEvent.change(screen.getByPlaceholderText("Search or create folder"), {
      target: { value: "clients/acme" },
    });

    expect(
      await screen.findByText("Enter a valid folder name."),
    ).not.toBeNull();
    expect(screen.queryByRole("option", { name: /Create/ })).toBeNull();
  });

  it("can remove the current note from its folder", () => {
    mocks.folderId = "work";

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Folder: work" }));
    fireEvent.click(screen.getByRole("option", { name: "No folder" }));

    expect(mocks.updateSession).toHaveBeenCalledWith({ folder_id: "" });
  });

  it("flattens a nested stored path when the top-level folder is selected", () => {
    mocks.folderId = "work/meetings";
    mocks.folderPaths = ["work"];

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Folder: work" }));
    fireEvent.click(screen.getByRole("option", { name: "work" }));

    expect(mocks.updateSession).toHaveBeenCalledWith({ folder_id: "work" });
  });
});
