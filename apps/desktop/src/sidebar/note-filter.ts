import { create } from "zustand";

export type SidebarNoteFilter = "mine";
export type SidebarNotesGroupBy = "date" | "folder";
export type SidebarNotesSortOrder = "newest" | "oldest";

type SidebarNotesState = {
  noteFilter: SidebarNoteFilter;
  folderFilter: string | null;
  groupBy: SidebarNotesGroupBy;
  sortOrder: SidebarNotesSortOrder;
  setView: (
    noteFilter: SidebarNoteFilter,
    folderFilter?: string | null,
  ) => void;
  setGroupBy: (groupBy: SidebarNotesGroupBy) => void;
  setSortOrder: (sortOrder: SidebarNotesSortOrder) => void;
};

const defaultSidebarNotes = {
  noteFilter: "mine" as const,
  folderFilter: null,
  groupBy: "date" as const,
  sortOrder: "newest" as const,
};

export const useSidebarNotes = create<SidebarNotesState>((set) => ({
  ...defaultSidebarNotes,
  setView: (noteFilter, folderFilter) =>
    set({
      noteFilter,
      folderFilter: folderFilter ?? null,
    }),
  setGroupBy: (groupBy) => set({ groupBy }),
  setSortOrder: (sortOrder) => set({ sortOrder }),
}));

export function resetSidebarNotes() {
  useSidebarNotes.setState({ ...defaultSidebarNotes });
}

export function folderIdForNewNote(
  folderFilter: string | null,
): string | undefined {
  return folderFilter ?? undefined;
}

export function encodeNotesView(folderFilter: string | null): string {
  if (folderFilter !== null) {
    return `folder:${folderFilter}`;
  }

  return "mine";
}

export function decodeNotesView(value: string): {
  noteFilter: SidebarNoteFilter;
  folderFilter: string | null;
} {
  if (value.startsWith("folder:")) {
    return { noteFilter: "mine", folderFilter: value.slice("folder:".length) };
  }

  return { noteFilter: "mine", folderFilter: null };
}
