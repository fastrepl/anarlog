import { create } from "zustand";

export type DevtoolsOtaPreviewStatus =
  | "available"
  | "downloading"
  | "ready"
  | "failed";

type ActiveDevtoolsOtaPreview = {
  status: DevtoolsOtaPreviewStatus;
  version: string;
  progress: number | null;
  key: number;
};

type DevtoolsOtaPreviewState = {
  preview: ActiveDevtoolsOtaPreview | null;
  showPreview: (status: DevtoolsOtaPreviewStatus) => void;
  clearPreview: () => void;
};

const DEVTOOLS_OTA_VERSION = "99.99.99-dev";

export const useDevtoolsOtaPreview = create<DevtoolsOtaPreviewState>((set) => ({
  preview: null,
  showPreview: (status) =>
    set((state) => ({
      preview: {
        status,
        version: DEVTOOLS_OTA_VERSION,
        progress: status === "downloading" ? 0.58 : null,
        key: (state.preview?.key ?? 0) + 1,
      },
    })),
  clearPreview: () => set({ preview: null }),
}));
