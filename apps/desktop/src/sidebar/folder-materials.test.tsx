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
  instructions: "",
  materials: [] as Array<{
    id: string;
    filename: string;
    contentType: string;
    sizeBytes: number;
    relativePath: string;
  }>,
  renameNamedFolder: vi.fn(),
  setView: vi.fn(),
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

vi.mock("~/session/folder-catalog", () => ({
  createNamedFolder: mocks.createNamedFolder,
  deleteNamedFolder: mocks.deleteNamedFolder,
  renameNamedFolder: mocks.renameNamedFolder,
  updateFolderInstructions: mocks.updateFolderInstructions,
  useFolderInstructions: () => mocks.instructions,
}));

vi.mock("./note-filter", () => ({
  useSidebarNotes: (
    selector: (state: { setView: typeof mocks.setView }) => unknown,
  ) => selector({ setView: mocks.setView }),
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

import { FolderMaterialsPanel } from "./folder-materials";

describe("FolderMaterialsPanel", () => {
  beforeEach(() => {
    mocks.createNamedFolder.mockReset();
    mocks.deleteLocalFolderMaterial.mockReset();
    mocks.deleteNamedFolder.mockReset();
    mocks.renameNamedFolder.mockReset();
    mocks.setView.mockReset();
    mocks.updateFolderInstructions.mockReset();
    mocks.upload.mockReset();
    mocks.instructions = "";
    mocks.materials = [];
    mocks.createNamedFolder.mockResolvedValue("CS 101/Week 1");
    mocks.deleteNamedFolder.mockResolvedValue(undefined);
    mocks.renameNamedFolder.mockResolvedValue("Algorithms");
    mocks.updateFolderInstructions.mockResolvedValue(undefined);
    mocks.upload.mockResolvedValue({
      path: "/vault/sessions/CS 101/materials/syllabus.txt",
      attachmentId: "syllabus.txt",
    });
    mocks.deleteLocalFolderMaterial.mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
  });

  it("uploads a selected file to the active folder", async () => {
    render(<FolderMaterialsPanel folderPath="CS 101" />);

    const file = new File(["week 1"], "syllabus.txt", { type: "text/plain" });
    fireEvent.change(document.querySelector('input[type="file"]')!, {
      target: { files: [file] },
    });

    await waitFor(() => {
      expect(mocks.upload).toHaveBeenCalledWith(file);
    });
  });

  it("removes a listed material", async () => {
    mocks.materials = [
      {
        id: "mat-1",
        filename: "syllabus.txt",
        contentType: "text/plain",
        sizeBytes: 12,
        relativePath: "materials/syllabus.txt",
      },
    ];

    render(<FolderMaterialsPanel folderPath="CS 101" />);
    expect(screen.getByText("syllabus.txt")).toBeTruthy();

    fireEvent.click(screen.getByLabelText("Remove syllabus.txt"));

    await waitFor(() => {
      expect(mocks.deleteLocalFolderMaterial).toHaveBeenCalledWith({
        folderPath: "CS 101",
        attachmentId: "syllabus.txt",
      });
    });
  });

  it("renames the active folder and keeps the sidebar on it", async () => {
    render(<FolderMaterialsPanel folderPath="CS 101" />);

    fireEvent.click(screen.getByLabelText("Rename folder"));
    fireEvent.change(screen.getByLabelText("Folder name"), {
      target: { value: "Algorithms" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));

    await waitFor(() => {
      expect(mocks.renameNamedFolder).toHaveBeenCalledWith(
        "CS 101",
        "Algorithms",
      );
      expect(mocks.setView).toHaveBeenCalledWith("mine", "Algorithms");
    });
  });

  it("creates a nested subfolder and switches to it", async () => {
    render(<FolderMaterialsPanel folderPath="CS 101" />);

    fireEvent.click(screen.getByLabelText("New subfolder"));
    fireEvent.change(screen.getByLabelText("Folder name"), {
      target: { value: "Week 1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => {
      expect(mocks.createNamedFolder).toHaveBeenCalledWith("CS 101/Week 1");
      expect(mocks.setView).toHaveBeenCalledWith("mine", "CS 101/Week 1");
    });
  });

  it("saves folder instructions when the field blurs", async () => {
    render(<FolderMaterialsPanel folderPath="CS 101" />);

    fireEvent.change(screen.getByLabelText("Folder instructions"), {
      target: { value: "Prefer the syllabus." },
    });
    fireEvent.blur(screen.getByLabelText("Folder instructions"));

    await waitFor(() => {
      expect(mocks.updateFolderInstructions).toHaveBeenCalledWith(
        "CS 101",
        "Prefer the syllabus.",
      );
    });
  });

  it("deletes the folder and returns to all notes", async () => {
    render(<FolderMaterialsPanel folderPath="CS 101" />);

    fireEvent.click(screen.getByLabelText("Delete folder"));
    fireEvent.click(screen.getByRole("button", { name: "Delete folder" }));

    await waitFor(() => {
      expect(mocks.deleteNamedFolder).toHaveBeenCalledWith("CS 101");
      expect(mocks.setView).toHaveBeenCalledWith("mine", null);
    });
  });
});
