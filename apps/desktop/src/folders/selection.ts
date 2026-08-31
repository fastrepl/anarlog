import { create } from "zustand";

export const useFolderSelection = create<{
  selectedPath: string | null;
  setSelectedPath: (path: string | null) => void;
}>((set) => ({
  selectedPath: null,
  setSelectedPath: (selectedPath) => set({ selectedPath }),
}));

export function useActiveFolderPath(folders: string[]): string | null {
  const selectedPath = useFolderSelection((state) => state.selectedPath);
  if (selectedPath !== null && folders.includes(selectedPath)) {
    return selectedPath;
  }
  return folders[0] ?? null;
}
