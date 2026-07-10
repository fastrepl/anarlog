import { sep } from "@tauri-apps/api/path";

import { commands as settingsCommands } from "@hypr/plugin-settings";

export const SESSION_META_FILE = "_meta.json";
export const SESSION_TRANSCRIPT_FILE = "transcript.json";
export const SESSION_NOTE_EXTENSION = ".md";
export const SESSION_MEMO_FILE = "_memo.md";

export async function getDataDir(): Promise<string> {
  const result = await settingsCommands.vaultBase();
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export function buildSessionPath(
  dataDir: string,
  sessionId: string,
  folderPath: string = "",
): string {
  if (folderPath) {
    const folderParts = folderPath.split("/");
    return [dataDir, "sessions", ...folderParts, sessionId].join(sep());
  }
  return [dataDir, "sessions", sessionId].join(sep());
}

export function buildEntityPath(dataDir: string, dirName: string): string {
  return [dataDir, dirName].join(sep());
}

export function buildEntityFilePath(
  dataDir: string,
  dirName: string,
  id: string,
): string {
  return [dataDir, dirName, `${id}.md`].join(sep());
}

export function getParentFolderPath(folderPath: string): string {
  if (!folderPath) {
    return "";
  }
  const parts = folderPath.split("/");
  parts.pop();
  return parts.join("/");
}

export function sanitizeFilename(name: string): string {
  return name.replace(/[<>:"/\\|?*]/g, "_").trim();
}

export function createMarkdownEntityParser(dirName: string) {
  return (path: string): string | null => {
    const parts = path.split("/");
    const dirIndex = parts.indexOf(dirName);
    if (dirIndex === -1 || dirIndex + 1 >= parts.length) {
      return null;
    }
    const filename = parts[dirIndex + 1];
    if (!filename?.endsWith(".md")) {
      return null;
    }
    return filename.slice(0, -3);
  };
}
