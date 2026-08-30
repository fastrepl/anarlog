import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FolderPicker } from "./folder-picker";

const mocks = vi.hoisted(() => ({
  createNamedFolder: vi.fn(() => Promise.resolve("clients")),
  folderId: "",
  folderPaths: [] as string[],
  updateSession: vi.fn(() => Promise.resolve()),
}));

vi.mock("~/session/folder-catalog", () => ({
  createNamedFolder: mocks.createNamedFolder,
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
    mocks.createNamedFolder.mockClear();
    mocks.updateSession.mockClear();
    mocks.createNamedFolder.mockResolvedValue("clients");
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
    expect(trigger.className).toContain("w-7");
    expect(trigger.className).toContain("[&_svg]:size-4");
    expect(icons).toHaveLength(1);
    expect(icons[0]?.getAttribute("class")).toContain("size-4");
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
    const classes = content?.className.split(/\s+/) ?? [];

    expect(classes).toContain("w-85");
    expect(classes).toContain("p-0.5");
    expect(classes).not.toContain("p-0");
    expect(input.closest(".p-4")).not.toBeNull();
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

    expect(mocks.createNamedFolder).toHaveBeenCalledWith("clients");
    await waitFor(() => {
      expect(mocks.updateSession).toHaveBeenCalledWith({
        folder_id: "clients",
      });
    });
  });

  it("creates nested folder names", async () => {
    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Select folder" }));
    fireEvent.change(screen.getByPlaceholderText("Search or create folder"), {
      target: { value: "clients/acme" },
    });
    fireEvent.click(
      await screen.findByRole("option", { name: 'Create "clients/acme"' }),
    );

    expect(mocks.createNamedFolder).toHaveBeenCalledWith("clients/acme");
    await waitFor(() => {
      expect(mocks.updateSession).toHaveBeenCalledWith({
        folder_id: "clients/acme",
      });
    });
  });

  it("can remove the current note from its folder", () => {
    mocks.folderId = "work";

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Folder: work" }));
    fireEvent.click(screen.getByRole("option", { name: "No folder" }));

    expect(mocks.updateSession).toHaveBeenCalledWith({ folder_id: "" });
  });

  it("keeps a nested stored path selected and can move the note to the parent", () => {
    mocks.folderId = "work/meetings";
    mocks.folderPaths = ["work", "work/meetings"];

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(
      screen.getByRole("combobox", { name: "Folder: work/meetings" }),
    );
    fireEvent.click(screen.getByRole("option", { name: "work" }));

    expect(mocks.updateSession).toHaveBeenCalledWith({ folder_id: "work" });
  });
});
