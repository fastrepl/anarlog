import { sep } from "@tauri-apps/api/path";

export function getSessionResourcePath(
  dataDir: string,
  sessionId: string,
): string {
  return [dataDir, "sessions", sessionId].join(sep());
}
