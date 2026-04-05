import type { SessionMode } from "~/store/zustand/listener/general-shared";

export type TabStatus =
  | "listening"
  | "listening-degraded"
  | "finalizing"
  | "processing";

export function getSessionTabStatus(
  sessionMode: SessionMode,
  isEnhancing: boolean,
  isDegraded: boolean,
  isSelected: boolean,
): TabStatus | undefined {
  if (sessionMode === "active") {
    return isDegraded ? "listening-degraded" : "listening";
  }
  if (sessionMode === "finalizing") {
    return "finalizing";
  }
  if (!isSelected && (isEnhancing || sessionMode === "running_batch")) {
    return "processing";
  }
  return undefined;
}
