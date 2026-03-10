import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";

import type { Store } from "~/store/tinybase/store/main";

export interface SessionOpsConfig {
  store: Store;
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
  const { store } = getConfig();
  const currentFolderId =
    (store.getCell("sessions", sessionId, "folder_id") as string | undefined) ??
    "";

  const result = await fsSyncCommands.moveSession(
    sessionId,
    currentFolderId,
    targetFolderId,
  );

  if (result.status === "error") {
    console.error("[SessionOps] moveSession failed:", result.error);
    return { status: "error", error: result.error };
  }

  store.setCell(
    "sessions",
    result.data.sessionId,
    "folder_id",
    result.data.folderId,
  );

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
    for (const update of result.data.updates) {
      store.setCell("sessions", update.sessionId, "folder_id", update.folderId);
    }
  });

  return { status: "ok" };
}

export const sessionOps = {
  moveSessionToFolder,
  renameFolder,
};
