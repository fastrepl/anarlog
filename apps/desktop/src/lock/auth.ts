import { commands } from "@anlg/plugin-local-auth";

export const DEVICE_AUTH_REASON = {
  openApp: "open",
  lockNote: "lock this note",
  unlockNote: "unlock this note",
  changeLockSettings: "change lock settings",
} as const;

export async function isDeviceAuthAvailable(): Promise<boolean> {
  const result = await commands.available();
  return result.status === "ok" && result.data;
}

export async function authenticateDevice(reason: string): Promise<boolean> {
  const result = await commands.authenticate(reason);
  return result.status === "ok" && result.data === true;
}
