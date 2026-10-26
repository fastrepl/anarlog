import { md2json } from "@anlg/editor/markdown";
import { toast } from "@anlg/ui/components/ui/toast";

import { createSession } from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";

const MARKDOWN_EXTENSION_PATTERN = /\.(md|markdown|mdown)$/i;

export function isMarkdownFile(file: Pick<File, "name">): boolean {
  return isMarkdownFileName(file.name);
}

export function isFileDrag(dataTransfer: DataTransfer): boolean {
  if (Array.from(dataTransfer.types ?? []).includes("Files")) {
    return true;
  }
  if (
    Array.from(dataTransfer.items ?? []).some((item) => item.kind === "file")
  ) {
    return true;
  }
  return Array.from(dataTransfer.files ?? []).length > 0;
}

export function markdownTitleFromFileName(fileName: string): string {
  const stripped = fileName.replace(/\.[^./\\]+$/, "");
  return stripped || fileName;
}

export type MarkdownDragMatch =
  | { active: false }
  | { active: true; title: string; subtitle: string };

export function matchMarkdownDrag(
  dataTransfer: DataTransfer,
): MarkdownDragMatch {
  const items = Array.from(dataTransfer.items ?? []);
  const names =
    items.length > 0
      ? allMarkdownItemNames(items)
      : allMarkdownFileNames(Array.from(dataTransfer.files ?? []));
  if (!names) {
    return { active: false };
  }
  return { active: true, ...overlayCopyForNames(names) };
}

export function markdownDragOverlayForPaths(
  paths: string[],
): MarkdownDragMatch {
  const names = paths.map((path) => path.split(/[\\/]/).pop() ?? path);
  if (names.length === 0 || !names.every(isMarkdownFileName)) {
    return { active: false };
  }
  return { active: true, ...overlayCopyForNames(names) };
}

function overlayCopyForNames(names: string[]): {
  title: string;
  subtitle: string;
} {
  return {
    subtitle:
      names.length === 1
        ? "Will be added as a new note"
        : `Will be added as ${names.length} new notes`,
    title:
      names.length === 1
        ? `Import "${names[0]}"`
        : `Import ${names.length} files`,
  };
}

function isMarkdownFileName(fileName: string): boolean {
  return MARKDOWN_EXTENSION_PATTERN.test(fileName);
}

export function matchMarkdownDrop(files: File[]): File[] | null {
  if (files.length === 0 || !files.every(isMarkdownFile)) {
    return null;
  }
  return files;
}

export async function importMarkdownFilesAsNotes(
  files: File[],
): Promise<string[]> {
  const imported: string[] = [];
  for (const file of files) {
    try {
      const text = await file.text();
      if (!text.trim()) {
        toast.error(`"${file.name}" is empty, skipped.`);
        continue;
      }
      const sessionId = await createSession(
        markdownTitleFromFileName(file.name),
        undefined,
        { raw_md: JSON.stringify(md2json(text)) },
      );
      imported.push(sessionId);
    } catch (error) {
      console.error("[markdown-import] failed to import", file.name, error);
      toast.error(`Could not import "${file.name}".`);
    }
  }
  if (imported.length > 0) {
    useTabs.getState().openNew({ type: "sessions", id: imported[0] });
    if (imported.length > 1) {
      toast.success(`Imported ${imported.length} notes.`);
    }
  }
  return imported;
}

function allMarkdownItemNames(items: DataTransferItem[]): string[] | null {
  if (items.length === 0) {
    return null;
  }
  const names: string[] = [];
  for (const item of items) {
    if (item.kind !== "file") {
      return null;
    }
    const file = item.getAsFile();
    if (!file || !isMarkdownFile(file)) {
      return null;
    }
    names.push(file.name);
  }
  return names;
}

function allMarkdownFileNames(files: File[]): string[] | null {
  if (files.length === 0 || !files.every(isMarkdownFile)) {
    return null;
  }
  return files.map((file) => file.name);
}
