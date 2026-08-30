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
  deleteLocalFolderMaterial: vi.fn(),
  materials: [] as Array<{
    id: string;
    filename: string;
    contentType: string;
    sizeBytes: number;
    relativePath: string;
  }>,
  renameNamedFolder: vi.fn(),
  setView: vi.fn(),
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
  renameNamedFolder: mocks.renameNamedFolder,
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
    mocks.deleteLocalFolderMaterial.mockReset();
    mocks.renameNamedFolder.mockReset();
    mocks.setView.mockReset();
    mocks.upload.mockReset();
    mocks.materials = [];
    mocks.renameNamedFolder.mockResolvedValue("Algorithms");
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
});
