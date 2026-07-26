import { create } from "zustand";

export type DevtoolsToastPreview =
  | "language-model"
  | "transcription-model"
  | "transcription-error"
  | "download"
  | "pro";

type ActiveDevtoolsToastPreview = {
  type: DevtoolsToastPreview;
  key: number;
};

type DevtoolsToastPreviewState = {
  preview: ActiveDevtoolsToastPreview | null;
  showPreview: (type: DevtoolsToastPreview) => void;
  clearPreview: () => void;
};

export const useDevtoolsToastPreview = create<DevtoolsToastPreviewState>(
  (set) => ({
    preview: null,
    showPreview: (type) =>
      set((state) => ({
        preview: {
          type,
          key: (state.preview?.key ?? 0) + 1,
        },
      })),
    clearPreview: () => set({ preview: null }),
  }),
);
