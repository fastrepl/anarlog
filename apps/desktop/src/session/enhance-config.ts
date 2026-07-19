import type { LLMConnectionStatus } from "~/ai/hooks";

export function shouldShowEmptySummaryConfigError(status: LLMConnectionStatus) {
  if (status.status === "pending") {
    return (
      status.reason === "missing_provider" || status.reason === "missing_model"
    );
  }

  if (status.status !== "error") {
    return false;
  }

  return (
    status.reason === "unauthenticated" ||
    status.reason === "not_pro" ||
    status.reason === "missing_config" ||
    status.reason === "google_calendar_data_check_failed" ||
    status.reason === "google_calendar_remote_ai_blocked"
  );
}
