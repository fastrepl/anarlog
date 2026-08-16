export type RecorderPhase =
  | "idle"
  | "starting"
  | "recording"
  | "saving"
  | "saved"
  | "unavailable"
  | "interrupted"
  | "save_error"
  | "error";

export type RecorderStatusFailure = {
  phase: "interrupted" | "error";
  reason: "media_services_reset" | "native_error";
  message: string;
};

export function recorderRecoveryAction(
  phase: RecorderPhase,
  hasPendingUri: boolean,
  intent: "stop" | "retry",
): "persist" | "stop" | "restart" | "noop" {
  if (hasPendingUri) return "persist";
  if (phase === "recording" || phase === "starting" || phase === "save_error") {
    return "stop";
  }
  if (
    intent === "retry" &&
    (phase === "unavailable" || phase === "interrupted" || phase === "error")
  ) {
    return "restart";
  }
  return "noop";
}

export function shouldHandleRecorderFailure(phase: RecorderPhase): boolean {
  return (
    phase === "starting" ||
    phase === "recording" ||
    phase === "interrupted" ||
    phase === "save_error" ||
    phase === "error"
  );
}

export function recoverableRecordingUri(
  statusUri: string | null | undefined,
  recorderUri: string | null | undefined,
): string | null {
  return statusUri || recorderUri || null;
}

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
