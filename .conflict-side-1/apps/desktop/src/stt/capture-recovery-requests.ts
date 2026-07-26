import { emit, listen } from "@tauri-apps/api/event";

const CAPTURE_RECOVERY_REQUEST_EVENT = "hypr:capture-recovery-request";

export function requestCaptureRecovery(sessionId: string): Promise<void> {
  return emit(CAPTURE_RECOVERY_REQUEST_EVENT, { sessionId });
}

export function listenCaptureRecoveryRequests(
  onRequest: (sessionId: string) => void,
) {
  return listen<unknown>(CAPTURE_RECOVERY_REQUEST_EVENT, ({ payload }) => {
    if (!payload || typeof payload !== "object") {
      return;
    }

    const sessionId = (payload as { sessionId?: unknown }).sessionId;
    if (typeof sessionId === "string" && sessionId.trim()) {
      onRequest(sessionId);
    }
  });
}
