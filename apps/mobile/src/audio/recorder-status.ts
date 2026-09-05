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
