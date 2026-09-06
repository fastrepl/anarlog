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

export function isRecordingStartCancelled(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    error.code === "ERR_AUDIO_STREAM_START_CANCELLED"
  );
}
