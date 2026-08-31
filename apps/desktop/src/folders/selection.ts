import { create } from "zustand";

import { type TemplateIcon } from "~/templates/template-icon";

export const useFolderSelection = create<{
  selectedPath: string | null;
  iconOverrides: Record<string, TemplateIcon>;
  setSelectedPath: (path: string | null) => void;
  setIconOverride: (path: string, icon: TemplateIcon) => void;
  rekeyIconOverride: (fromPath: string, toPath: string) => void;
}>((set) => ({
  selectedPath: null,
  iconOverrides: {},
  setSelectedPath: (selectedPath) => set({ selectedPath }),
  setIconOverride: (path, icon) =>
    set((state) => ({
      iconOverrides: { ...state.iconOverrides, [path]: icon },
    })),
  rekeyIconOverride: (fromPath, toPath) =>
    set((state) => {
      if (fromPath === toPath || !state.iconOverrides[fromPath]) {
        return state;
      }
      const { [fromPath]: icon, ...rest } = state.iconOverrides;
      return { iconOverrides: { ...rest, [toPath]: icon } };
    }),
}));

export function useActiveFolderPath(folders: string[]): string | null {
  const selectedPath = useFolderSelection((state) => state.selectedPath);
  if (selectedPath !== null) {
    return selectedPath;
  }
  return folders[0] ?? null;
}
