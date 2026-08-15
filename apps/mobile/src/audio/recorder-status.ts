export type RecorderStatusFailure = {
  phase: "interrupted" | "error";
  reason: "media_services_reset" | "native_error";
  message: string;
};

export function recorderStatusFailure(status: {
  error: string | null;
  hasError: boolean;
  mediaServicesDidReset?: boolean;
}): RecorderStatusFailure | null {
  if (status.mediaServicesDidReset) {
    return {
      phase: "interrupted",
      reason: "media_services_reset",
      message: "Audio media services reset",
    };
  }
  if (status.hasError) {
    return {
      phase: "error",
      reason: "native_error",
      message: status.error ?? "Native recording failed",
    };
  }
  return null;
}
