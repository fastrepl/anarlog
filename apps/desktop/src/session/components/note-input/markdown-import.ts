import {
  type DragEvent,
  type HTMLAttributes,
  useCallback,
  useMemo,
} from "react";

import { md2json } from "@anlg/editor/markdown";
import { toast } from "@anlg/ui/components/ui/toast";

import { createSession } from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";

const MARKDOWN_EXTENSION_PATTERN = /\.(md|markdown|mdown)$/i;

export function useMarkdownImportDrop() {
  const handleDragOver = useCallback((event: DragEvent<HTMLDivElement>) => {
    if (!isFileDrag(event.dataTransfer)) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  }, []);

  const handleDrop = useCallback((event: DragEvent<HTMLDivElement>) => {
    const files = Array.from(event.dataTransfer.files ?? []);
    if (files.length === 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const matched = matchMarkdownDrop(files);
    if (matched) {
      void importMarkdownFilesAsNotes(matched);
    }
  }, []);

  return useMemo(
    () => ({
      markdownImportDropProps: {
        onDragOver: handleDragOver,
        onDrop: handleDrop,
      } satisfies HTMLAttributes<HTMLDivElement>,
    }),
    [handleDragOver, handleDrop],
  );
}

export function isMarkdownFile(file: Pick<File, "name">): boolean {
  return MARKDOWN_EXTENSION_PATTERN.test(file.name);
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
