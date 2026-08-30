import type { SidebarNotesGroupBy } from "./note-filter";

import { normalizeFolderPath } from "~/session/folders";

export function parseSessionTagNames(
  value: string | null | undefined,
): string[] {
  if (!value) {
    return [];
  }

  try {
    const parsed = JSON.parse(value);
    if (!Array.isArray(parsed)) {
      return [];
    }

    const names = new Set<string>();
    for (const entry of parsed) {
      if (typeof entry !== "string") {
        continue;
      }
      const name = entry.trim();
      if (name) {
        names.add(name);
      }
    }

    return [...names].sort((left, right) => left.localeCompare(right));
  } catch {
    return [];
  }
}

export function resolveSidebarItemMeta({
  folderId,
  tags,
  showFolder,
  showTags,
  groupBy,
}: {
  folderId?: string | null;
  tags?: string[] | null;
  showFolder: boolean;
  showTags: boolean;
  groupBy: SidebarNotesGroupBy;
}): { folder: string; tags: string[] } {
  return {
    folder:
      showFolder && groupBy !== "folder"
        ? (normalizeFolderPath(folderId ?? "") ?? "")
        : "",
    tags: showTags
      ? [...new Set((tags ?? []).map((tag) => tag.trim()).filter(Boolean))]
      : [],
  };
}

export function formatSidebarItemMetaLine(
  folder: string,
  tags: string[],
): string {
  const tagLabel = tags.map((tag) => `#${tag}`).join(" ");
  if (folder && tagLabel) {
    return `${folder} · ${tagLabel}`;
  }

  return folder || tagLabel;
}
