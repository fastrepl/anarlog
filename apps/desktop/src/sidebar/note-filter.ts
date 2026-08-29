import { create } from "zustand";

export type SidebarNoteFilter = "mine" | "shared";

type SidebarNotesState = {
  noteFilter: SidebarNoteFilter;
  folderFilter: string | null;
  setView: (
    noteFilter: SidebarNoteFilter,
    folderFilter?: string | null,
  ) => void;
};

export const useSidebarNotes = create<SidebarNotesState>((set) => ({
  noteFilter: "mine",
  folderFilter: null,
  setView: (noteFilter, folderFilter) =>
    set({
      noteFilter,
      folderFilter: noteFilter === "shared" ? null : (folderFilter ?? null),
    }),
}));

export function resetSidebarNotes() {
  useSidebarNotes.setState({
    noteFilter: "mine",
    folderFilter: null,
  });
}

export function folderIdForNewNote(
  noteFilter: SidebarNoteFilter,
  folderFilter: string | null,
): string | undefined {
  if (noteFilter !== "mine" || folderFilter === null) {
    return undefined;
  }

  return folderFilter;
}

export function encodeNotesView(
  noteFilter: SidebarNoteFilter,
  folderFilter: string | null,
): string {
  if (noteFilter === "shared") {
    return "shared";
  }

  if (folderFilter !== null) {
    return `folder:${folderFilter}`;
  }

  return "mine";
}

export function decodeNotesView(value: string): {
  noteFilter: SidebarNoteFilter;
  folderFilter: string | null;
} {
  if (value === "shared") {
    return { noteFilter: "shared", folderFilter: null };
  }

  if (value.startsWith("folder:")) {
    return { noteFilter: "mine", folderFilter: value.slice("folder:".length) };
  }

  return { noteFilter: "mine", folderFilter: null };
}
