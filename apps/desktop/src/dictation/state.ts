import { create } from "zustand";

import type { DictationPhase } from "./controller";

export const useDictationStatus = create<{
  phase: DictationPhase;
  error: string | null;
  lastTranscript: string;
  ready: boolean;
  retry: number;
  owner: string | null;
  presentedOwner: string | null;
  microphone: string;
  amplitude: number;
  text: string;
  partial: string;
  previewEnabled: boolean;
  previewUnavailable: boolean;
  expanded: boolean;
  cancel: (() => void) | null;
  finish: (() => void) | null;
}>(() => ({
  phase: "idle",
  error: null,
  lastTranscript: "",
  ready: false,
  retry: 0,
  owner: null,
  presentedOwner: null,
  microphone: "Default microphone",
  amplitude: 0,
  text: "",
  partial: "",
  previewEnabled: false,
  previewUnavailable: false,
  expanded: false,
  cancel: null,
  finish: null,
}));
