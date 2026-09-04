import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { completeDestructiveButtonHold } from "~/test-utils/destructive-button";

const mocks = vi.hoisted(() => ({
  createNamedFolder: vi.fn(),
  deleteLocalFolderMaterial: vi.fn(),
  deleteNamedFolder: vi.fn(),
  folders: [] as string[],
  icons: {} as Record<string, { type: "icon"; value: string; color: string }>,
  instructions: "",
  materials: [] as Array<{
    id: string;
    filename: string;
    contentType: string;
    sizeBytes: number;
    relativePath: string;
  }>,
  renameNamedFolder: vi.fn(),
  updateFolderIcon: vi.fn(),
  updateFolderInstructions: vi.fn(),
  upload: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

vi.mock("~/session/queries", () => ({
  useFolderIcons: () => mocks.icons,
  useFolderPaths: () => mocks.folders,
}));

vi.mock("~/session/folder-catalog", () => ({
  createNamedFolder: mocks.createNamedFolder,
  deleteNamedFolder: mocks.deleteNamedFolder,
  renameNamedFolder: mocks.renameNamedFolder,
  updateFolderIcon: mocks.updateFolderIcon,
  updateFolderInstructions: mocks.updateFolderInstructions,
  useFolderInstructions: () => mocks.instructions,
}));

vi.mock("~/session/folder-attachments", () => ({
  deleteLocalFolderMaterial: mocks.deleteLocalFolderMaterial,
  diskAttachmentId: (relativePath: string) => {
    const parts = relativePath.split("/");
    return parts[parts.length - 1] ?? relativePath;
  },
  useFolderMaterials: () => mocks.materials,
}));

vi.mock("~/shared/hooks/useFileUpload", () => ({
  useFolderMaterialUpload: () => mocks.upload,
}));

vi.mock("~/sidebar/custom-sidebar-header", () => ({
  CustomSidebarHeader: ({ children }: { children?: ReactNode }) => (
    <div>{children}</div>
  ),
}));

import { FoldersMain } from "./index";
import { useFolderSelection } from "./selection";
import { FoldersSidebar } from "./sidebar";

function FoldersWorkspace() {
  return (
    <>
      <FoldersSidebar />
      <FoldersMain />
    </>
  );
}

function renderFoldersWorkspace() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <FoldersWorkspace />
    </QueryClientProvider>,
  );
}

describe("Folders workspace", () => {
  beforeEach(() => {
    mocks.createNamedFolder.mockReset();
    mocks.deleteLocalFolderMaterial.mockReset();
    mocks.deleteNamedFolder.mockReset();
    mocks.renameNamedFolder.mockReset();
    mocks.updateFolderIcon.mockReset();
    mocks.updateFolderInstructions.mockReset();
    mocks.upload.mockReset();
    mocks.folders = [];
    mocks.icons = {};
    mocks.instructions = "";
    mocks.materials = [];
    mocks.createNamedFolder.mockResolvedValue("CS 101");
    mocks.deleteNamedFolder.mockResolvedValue(undefined);
    mocks.renameNamedFolder.mockResolvedValue("Algorithms");
    mocks.updateFolderIcon.mockResolvedValue(undefined);
    mocks.updateFolderInstructions.mockResolvedValue(undefined);
    mocks.upload.mockResolvedValue({
      path: "/vault/sessions/CS 101/materials/syllabus.pdf",
      attachmentId: "syllabus.pdf",
    });
    mocks.deleteLocalFolderMaterial.mockResolvedValue(undefined);
    useFolderSelection.setState({
      selectedPath: null,
      deletedPrefixes: [],
      iconOverrides: {},
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows an empty state until a folder is created", async () => {
    renderFoldersWorkspace();

    expect(
      screen.getByText(
        "No folders yet. Create one to group notes and materials.",
      ),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "New folder" }));
    fireEvent.change(screen.getByLabelText("Folder name"), {
      target: { value: "CS 101" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(mocks.createNamedFolder).toHaveBeenCalledWith("CS 101");
    });
  });

  it("edits context and materials for the selected folder", async () => {
    mocks.folders = ["CS 101", "Work"];
    mocks.materials = [
      {
        id: "mat-1",
        filename: "syllabus.pdf",
        contentType: "application/pdf",
        sizeBytes: 12,
        relativePath: "materials/syllabus.pdf",
      },
    ];

    renderFoldersWorkspace();

    expect(screen.getByRole("textbox", { name: "Folder name" })).toHaveProperty(
      "value",
      "CS 101",
    );
    fireEvent.click(screen.getByRole("button", { name: "Work" }));
    expect(screen.getByRole("textbox", { name: "Folder name" })).toHaveProperty(
      "value",
      "Work",
    );

    fireEvent.change(screen.getByLabelText("Folder context"), {
      target: { value: "Prefer the syllabus." },
    });
    fireEvent.blur(screen.getByLabelText("Folder context"));

    await waitFor(() => {
      expect(mocks.updateFolderInstructions).toHaveBeenCalledWith(
        "Work",
        "Prefer the syllabus.",
      );
    });

    expect(screen.getByText("Context")).toBeTruthy();
    expect(screen.getByText("What these notes are usually about")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add file" })).toBeTruthy();
    expect(screen.getByText("syllabus.pdf")).toBeTruthy();
    const file = new File(["week 1"], "notes.txt", { type: "text/plain" });
    fireEvent.click(screen.getByRole("button", { name: "Add file" }));
    fireEvent.change(document.querySelector('input[type="file"]')!, {
      target: { files: [file] },
    });

    await waitFor(() => {
      expect(mocks.upload).toHaveBeenCalledWith(file);
    });
  });

  it("filters the sidebar by folder name", () => {
    mocks.folders = ["CS 101", "Work"];

    renderFoldersWorkspace();

    fireEvent.change(screen.getByPlaceholderText("Search folders..."), {
      target: { value: "work" },
    });

    expect(screen.getByRole("button", { name: "Work" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "CS 101" })).toBeNull();
  });

  it("shows parent paths for nested folders", () => {
    mocks.folders = ["Work/Sales", "Personal/Sales"];

    renderFoldersWorkspace();

    expect(screen.getByText("Work/Sales")).toBeTruthy();
    expect(screen.getByText("Personal/Sales")).toBeTruthy();
  });

  it("renames the folder from the title field", async () => {
    mocks.folders = ["Work"];

    renderFoldersWorkspace();

    expect(screen.getByRole("button", { name: "Add file" })).toBeTruthy();
    expect(
      screen.queryByText("Add a syllabus or PDF for this folder"),
    ).toBeNull();

    const title = screen.getByRole("textbox", { name: "Folder name" });
    fireEvent.change(title, { target: { value: "Algorithms" } });
    fireEvent.blur(title);

    await waitFor(() => {
      expect(mocks.renameNamedFolder).toHaveBeenCalledWith(
        "Work",
        "Algorithms",
      );
    });
  });

  it("keeps an explicitly selected folder active while queries catch up", () => {
    mocks.folders = ["Work"];
    useFolderSelection.setState({ selectedPath: "New Folder" });

    renderFoldersWorkspace();

    expect(screen.getByRole("textbox", { name: "Folder name" })).toHaveProperty(
      "value",
      "New Folder",
    );
  });

  it("keeps a nested folder under its parent when renaming it", async () => {
    mocks.folders = ["Courses/Algorithms"];
    mocks.renameNamedFolder.mockResolvedValue("Courses/Data Structures");

    renderFoldersWorkspace();

    const title = screen.getByRole("textbox", { name: "Folder name" });
    fireEvent.change(title, { target: { value: "Data Structures" } });
    fireEvent.blur(title);

    await waitFor(() => {
      expect(mocks.renameNamedFolder).toHaveBeenCalledWith(
        "Courses/Algorithms",
        "Courses/Data Structures",
      );
    });
  });

  it("deletes the folder from the actions menu", async () => {
    mocks.folders = ["Work", "Personal"];

    renderFoldersWorkspace();

    fireEvent.pointerDown(
      screen.getByRole("button", { name: "Folder actions" }),
      { button: 0, ctrlKey: false },
    );
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
    expect(
      screen.getByText(
        "Notes stay in All notes. This folder, its nested folders, and all their materials will be deleted.",
      ),
    ).toBeTruthy();
    completeDestructiveButtonHold(
      screen.getByRole("button", { name: "Delete folder" }),
    );

    await waitFor(() => {
      expect(mocks.deleteNamedFolder).toHaveBeenCalledWith("Work");
      expect(
        screen.getByRole("textbox", { name: "Folder name" }),
      ).toHaveProperty("value", "Personal");
    });
  });

  it("saves a folder icon from the header picker", async () => {
    mocks.folders = ["Work"];

    renderFoldersWorkspace();

    fireEvent.click(screen.getByRole("button", { name: "Choose folder icon" }));
    fireEvent.click(screen.getByRole("button", { name: "target" }));

    await waitFor(() => {
      expect(mocks.updateFolderIcon).toHaveBeenCalledWith("Work", {
        type: "icon",
        value: "target",
        color: "#9ca3af",
      });
    });
    expect(useFolderSelection.getState().iconOverrides.Work).toEqual({
      type: "icon",
      value: "target",
      color: "#9ca3af",
    });
  });

  it("clears an optimistic folder icon when saving fails", async () => {
    mocks.folders = ["Work"];
    mocks.updateFolderIcon.mockRejectedValue(new Error("unavailable"));
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    renderFoldersWorkspace();

    fireEvent.click(screen.getByRole("button", { name: "Choose folder icon" }));
    fireEvent.click(screen.getByRole("button", { name: "target" }));

    await waitFor(() => {
      expect(mocks.updateFolderIcon).toHaveBeenCalledWith("Work", {
        type: "icon",
        value: "target",
        color: "#9ca3af",
      });
      expect(useFolderSelection.getState().iconOverrides.Work).toBeUndefined();
    });
    consoleError.mockRestore();
  });
});
