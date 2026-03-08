import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import type { Store } from "~/store/tinybase/store/main";

export interface SessionOpsConfig {
  store: Store;
  reloadSessions: () => Promise<void>;
}

let config: SessionOpsConfig | null = null;

export function initSessionOps(cfg: SessionOpsConfig) {
  config = cfg;
}

function getConfig(): SessionOpsConfig {
  if (!config) {
    throw new Error("[SessionOps] Not initialized. Call initSessionOps first.");
  }
  return config;
}

export async function moveSessionToFolder(
  sessionId: string,
  targetFolderId: string,
): Promise<{ status: "ok" } | { status: "error"; error: string }> {
  const { store, reloadSessions } = getConfig();

  store.setCell("sessions", sessionId, "folder_id", targetFolderId);

  const result = await fsSyncCommands.moveSession(sessionId, targetFolderId);

  if (result.status === "error") {
    console.error("[SessionOps] moveSession failed:", result.error);
    await reloadSessions();
    return { status: "error", error: result.error };
  }

  return { status: "ok" };
}

export async function renameFolder(
  oldPath: string,
  newPath: string,
): Promise<{ status: "ok" } | { status: "error"; error: string }> {
  const { store } = getConfig();

  const result = await fsSyncCommands.renameFolder(oldPath, newPath);

  if (result.status === "error") {
    console.error("[SessionOps] renameFolder failed:", result.error);
    return { status: "error", error: result.error };
  }

  store.transaction(() => {
    const sessionIds = store.getRowIds("sessions");
    for (const id of sessionIds) {
      const folderId = store.getCell("sessions", id, "folder_id");
      if (folderId === oldPath) {
        store.setCell("sessions", id, "folder_id", newPath);
      } else if (folderId?.startsWith(oldPath + "/")) {
        store.setCell(
          "sessions",
          id,
          "folder_id",
          folderId.replace(oldPath, newPath),
        );
      }
    }
  });

  return { status: "ok" };
}

export const sessionOps = {
  moveSessionToFolder,
  renameFolder,
};
