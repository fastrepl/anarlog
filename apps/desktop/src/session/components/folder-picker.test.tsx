import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FolderPicker, FolderPickerContent } from "./folder-picker";

const mocks = vi.hoisted(() => ({
  createNamedFolder: vi.fn(() => Promise.resolve("clients")),
  folderId: "",
  folderPaths: [] as string[],
  icons: {} as Record<string, { type: "icon"; value: string; color: string }>,
  onSelectFolder: vi.fn(() => Promise.resolve()),
  openNew: vi.fn(),
  setSelectedPath: vi.fn(),
  updateSession: vi.fn(() => Promise.resolve()),
  windowLabel: "main",
  windowEmitNavigate: vi.fn(() =>
    Promise.resolve({ status: "ok", data: null }),
  ),
  windowShow: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
}));

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    windowEmitNavigate: mocks.windowEmitNavigate,
    windowShow: mocks.windowShow,
  },
  getCurrentWebviewWindowLabel: () => mocks.windowLabel,
}));

vi.mock("~/folders/selection", () => ({
  useFolderSelection: (
    selector: (state: {
      setSelectedPath: typeof mocks.setSelectedPath;
    }) => unknown,
  ) => selector({ setSelectedPath: mocks.setSelectedPath }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof mocks.openNew }) => unknown) =>
    selector({ openNew: mocks.openNew }),
}));

vi.mock("~/session/folder-catalog", () => ({
  createNamedFolder: mocks.createNamedFolder,
}));

vi.mock("~/session/queries", () => ({
  useFolderIcons: () => mocks.icons,
  useFolderPaths: () => mocks.folderPaths,
  useSession: () => ({ folder_id: mocks.folderId }),
  useUpdateSession: () => mocks.updateSession,
}));

describe("FolderPicker", () => {
  beforeEach(() => {
    mocks.folderId = "";
    mocks.folderPaths = ["personal", "work"];
    mocks.icons = {};
    mocks.createNamedFolder.mockClear();
    mocks.onSelectFolder.mockClear();
    mocks.openNew.mockClear();
    mocks.setSelectedPath.mockClear();
    mocks.updateSession.mockClear();
    mocks.windowLabel = "main";
    mocks.windowEmitNavigate.mockClear();
    mocks.windowShow.mockClear();
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

  it("shows the selected folder name", () => {
    mocks.folderId = "work";

    render(<FolderPicker sessionId="session-1" />);

    const trigger = screen.getByRole("combobox", { name: "Folder: work" });

    expect(trigger.textContent).toBe("work");
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
      expect(mocks.setSelectedPath).toHaveBeenCalledWith("clients");
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

  it("highlights the current folder and does not offer no folder", () => {
    mocks.folderId = "work";

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Folder: work" }));

    expect(
      screen
        .getByRole("option", { name: "work" })
        .getAttribute("data-selected"),
    ).toBe("true");
    expect(screen.queryByRole("option", { name: "No folder" })).toBeNull();
  });

  it("can remove the current note from its folder", () => {
    mocks.folderId = "work";

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Folder: work" }));
    fireEvent.click(screen.getByRole("option", { name: "work" }));

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

  it("routes selection through onSelectFolder and highlights selectedPath when overridden", async () => {
    render(
      <FolderPickerContent
        sessionId="session-1"
        onClose={() => {}}
        selectedPath="work"
        onSelectFolder={mocks.onSelectFolder}
      />,
    );

    expect(
      screen
        .getByRole("option", { name: "work" })
        .getAttribute("data-selected"),
    ).toBe("true");
    fireEvent.click(screen.getByRole("option", { name: "personal" }));

    await waitFor(() => {
      expect(mocks.onSelectFolder).toHaveBeenCalledWith("personal");
    });
    expect(mocks.updateSession).not.toHaveBeenCalled();
  });

  it("opens the folders workspace from see all folders", () => {
    mocks.folderId = "work";

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Folder: work" }));
    fireEvent.click(screen.getByRole("button", { name: "See all folders" }));

    expect(mocks.setSelectedPath).toHaveBeenCalledWith("work");
    expect(mocks.openNew).toHaveBeenCalledWith({ type: "folders" });
    expect(mocks.windowEmitNavigate).not.toHaveBeenCalled();
  });

  it("opens the main window folders workspace from a note window", async () => {
    mocks.windowLabel = "note-session-1";
    mocks.folderId = "work";

    render(<FolderPicker sessionId="session-1" />);

    fireEvent.click(screen.getByRole("combobox", { name: "Folder: work" }));
    fireEvent.click(screen.getByRole("button", { name: "See all folders" }));

    await waitFor(() => {
      expect(mocks.windowEmitNavigate).toHaveBeenCalledWith(
        { type: "main" },
        { path: "/app/folders", search: { path: "work" } },
      );
      expect(mocks.windowShow).toHaveBeenCalledWith({ type: "main" });
    });
    // The main window must exist before the navigation event is emitted,
    // otherwise a startup-time emit can be dropped before its React
    // navigation listener has registered.
    expect(mocks.windowShow.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.windowEmitNavigate.mock.invocationCallOrder[0]!,
    );
    expect(mocks.openNew).not.toHaveBeenCalled();
    expect(mocks.setSelectedPath).not.toHaveBeenCalled();
  });
});
