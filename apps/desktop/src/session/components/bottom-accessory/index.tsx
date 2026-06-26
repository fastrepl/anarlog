import type { ReactNode } from "react";

export type BottomAccessoryState = {
  mode: "playback" | "live";
  expanded: boolean;
} | null;

export function useSessionBottomAccessory(_params: {
  sessionId: string;
  sessionMode: string;
  audioExists?: boolean;
  audioUrlReady?: boolean;
  isAudioLoading?: boolean;
  hasTranscript?: boolean;
}): {
  bottomAccessory: ReactNode;
  bottomBorderHandle: ReactNode;
  bottomAccessoryState: BottomAccessoryState;
} {
  return {
    bottomAccessory: null,
    bottomBorderHandle: null,
    bottomAccessoryState: null,
  };
}
