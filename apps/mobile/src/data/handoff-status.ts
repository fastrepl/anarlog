export type HandoffStatus =
  | "local"
  | "sharing"
  | "shared_unconfirmed"
  | "unavailable"
  | "failed";

export function handoffStatusCopy(status: HandoffStatus): string {
  switch (status) {
    case "sharing":
      return "Opening the system share sheet…";
    case "shared_unconfirmed":
      return "Share sheet closed. Import on desktop is not confirmed.";
    case "unavailable":
      return "File sharing is unavailable on this device.";
    case "failed":
      return "Handoff failed. The recording is still safe on this phone.";
    default:
      return "On this phone. Send the audio file to import it on desktop.";
  }
}
