import type { LLMConnectionStatus } from "~/ai/hooks";

export function shouldShowEmptySummaryConfigError(status: LLMConnectionStatus) {
  if (status.status !== "error") {
    return false;
  }

  return (
    status.reason === "unauthenticated" ||
    status.reason === "not_pro" ||
    status.reason === "missing_config"
  );
}
