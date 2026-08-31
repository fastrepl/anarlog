import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  createNamedFolder: vi.fn(),
  deleteLocalFolderMaterial: vi.fn(),
  deleteNamedFolder: vi.fn(),
  folders: [] as string[],
  instructions: "",
  materials: [] as Array<{
    id: string;
    filename: string;
    contentType: string;
    sizeBytes: number;
    relativePath: string;
  }>,
  renameNamedFolder: vi.fn(),
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
  useFolderPaths: () => mocks.folders,
}));

vi.mock("~/session/folder-catalog", () => ({
  createNamedFolder: mocks.createNamedFolder,
  deleteNamedFolder: mocks.deleteNamedFolder,
  renameNamedFolder: mocks.renameNamedFolder,
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

describe("Folders workspace", () => {
  beforeEach(() => {
    mocks.createNamedFolder.mockReset();
    mocks.deleteLocalFolderMaterial.mockReset();
    mocks.deleteNamedFolder.mockReset();
    mocks.renameNamedFolder.mockReset();
    mocks.updateFolderInstructions.mockReset();
    mocks.upload.mockReset();
    mocks.folders = [];
    mocks.instructions = "";
    mocks.materials = [];
    mocks.createNamedFolder.mockResolvedValue("CS 101");
    mocks.deleteNamedFolder.mockResolvedValue(undefined);
    mocks.renameNamedFolder.mockResolvedValue("Algorithms");
    mocks.updateFolderInstructions.mockResolvedValue(undefined);
    mocks.upload.mockResolvedValue({
      path: "/vault/sessions/CS 101/materials/syllabus.pdf",
      attachmentId: "syllabus.pdf",
    });
    mocks.deleteLocalFolderMaterial.mockResolvedValue(undefined);
    useFolderSelection.setState({ selectedPath: null });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows an empty state until a folder is created", async () => {
    render(<FoldersWorkspace />);

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

  it("edits instructions and materials for the selected folder", async () => {
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

    render(<FoldersWorkspace />);

    expect(screen.getByRole("heading", { name: "CS 101" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Work" }));
    expect(screen.getByRole("heading", { name: "Work" })).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Folder instructions"), {
      target: { value: "Prefer the syllabus." },
    });
    fireEvent.blur(screen.getByLabelText("Folder instructions"));

    await waitFor(() => {
      expect(mocks.updateFolderInstructions).toHaveBeenCalledWith(
        "Work",
        "Prefer the syllabus.",
      );
    });

    expect(screen.getByText("syllabus.pdf")).toBeTruthy();
    const file = new File(["week 1"], "notes.txt", { type: "text/plain" });
    fireEvent.change(document.querySelector('input[type="file"]')!, {
      target: { files: [file] },
    });

    await waitFor(() => {
      expect(mocks.upload).toHaveBeenCalledWith(file);
    });
  });
});
