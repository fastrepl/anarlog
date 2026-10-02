import { create } from "zustand";

import type { LocalModel } from "@anlg/plugin-local-stt";

export const usePendingSttSelection = create<{
  selection: { provider: string; model: LocalModel } | null;
  queuedDownloads: LocalModel[];
}>()(() => ({ selection: null, queuedDownloads: [] }));
