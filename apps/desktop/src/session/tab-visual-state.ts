import type { SessionMode } from "~/store/zustand/listener/general-shared";

export function getSessionTabVisualState(
  sessionMode: SessionMode,
  isEnhancing: boolean,
  isSelected: boolean,
) {
  const isListening = sessionMode === "active" || sessionMode === "finalizing";
  const isFinalizing = sessionMode === "finalizing";
  const isBatching = sessionMode === "running_batch";

  return {
    isActive: isListening,
    accent: sessionMode === "active" ? "red" : "neutral",
    showSpinner: isFinalizing || (!isSelected && (isEnhancing || isBatching)),
  } as const;
}
