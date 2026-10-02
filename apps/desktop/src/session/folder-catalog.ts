import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands } from "@anlg/plugin-session";

import { ancestorFolderPaths, normalizeFolderPath } from "./folders";

import { liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { normalizeFolderIcon } from "~/session/folder-icon";
import { type TemplateIcon } from "~/templates/template-icon";

export async function ensureFolderCatalog(folderPath: string): Promise<string> {
  const path = requireNamedFolderPath(folderPath);
  await enqueueDatabaseWrite("folders", async () => {
    const result = await commands.ensureFolderCatalog({
      paths: ancestorFolderPaths(path),
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
  return path;
}

export async function createNamedFolder(folderPath: string): Promise<string> {
  const path = await ensureFolderCatalog(folderPath);
  const result = await fsSyncCommands.createFolder(path);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return path;
}

export async function renameNamedFolder(
  oldFolderPath: string,
  newFolderPath: string,
): Promise<string> {
  const oldPath = requireNamedFolderPath(oldFolderPath);
  const newPath = requireNamedFolderPath(newFolderPath);
  if (oldPath === newPath) {
    return newPath;
  }

  if (await folderNameTaken(newPath)) {
    throw new Error("folder_target_exists");
  }

  await renameFolderOnDisk(oldPath, newPath);

  await enqueueDatabaseWrite("folders", async () => {
    const result = await commands.renameFolderCatalog({
      old_path: oldPath,
      new_path: newPath,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });

  return newPath;
}

export async function deleteNamedFolder(folderPath: string): Promise<void> {
  const path = requireNamedFolderPath(folderPath);
  const sessions = await liveQueryClient.execute<{
    id: string;
    folder_path: string;
  }>(
    `
      SELECT id, folder_path
      FROM sessions
      WHERE deleted_at IS NULL
        AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
    `,
    [path, `${path}/%`, `${path}\\%`],
  );

  for (const session of sessions) {
    const moved = await fsSyncCommands.moveSession(
      session.id,
      session.folder_path,
      "",
    );
    if (
      moved.status === "error" &&
      !String(moved.error).includes("session_source_missing")
    ) {
      throw new Error(moved.error);
    }
  }

  await enqueueDatabaseWrite("folders", async () => {
    const result = await commands.deleteFolderCatalog({ path });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });

  const deleted = await fsSyncCommands.deleteFolder(path);
  if (
    deleted.status === "error" &&
    !String(deleted.error).includes("folder_source_missing")
  ) {
    throw new Error(deleted.error);
  }
}

export async function updateFolderInstructions(
  folderPath: string,
  instructions: string,
): Promise<void> {
  const path = await ensureFolderCatalog(folderPath);
  await enqueueDatabaseWrite("folders", async () => {
    const result = await commands.updateFolderInstructions({
      path,
      instructions,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export async function updateFolderWorkspace(
  folderPath: string,
  workspaceId: string,
): Promise<void> {
  const path = await ensureFolderCatalog(folderPath);
  await enqueueDatabaseWrite("folders", async () => {
    const result = await commands.updateFolderWorkspace({
      path,
      workspace_id: workspaceId,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export async function updateFolderIcon(
  folderPath: string,
  icon: TemplateIcon,
): Promise<void> {
  const path = requireNamedFolderPath(folderPath);
  const iconJson = JSON.stringify(normalizeFolderIcon(icon));
  await enqueueDatabaseWrite("folders", async () => {
    const result = await commands.updateFolderIcon({
      path,
      icon_json: iconJson,
    });
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export async function loadFolderInstructions(
  folderPath: string,
): Promise<string> {
  const path = normalizeFolderPath(folderPath);
  if (!path) {
    return "";
  }

  const rows = await liveQueryClient.execute<{ instructions: string }>(
    `
      SELECT instructions
      FROM folders
      WHERE path = ?
        AND deleted_at IS NULL
      LIMIT 1
    `,
    [path],
  );
  return rows[0]?.instructions ?? "";
}

export function useFolderInstructions(folderPath: string): string {
  const { data = "" } = useLiveQuery<{ instructions: string }, string>({
    sql: `
      SELECT instructions
      FROM folders
      WHERE path = ?
        AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [folderPath],
    enabled: folderPath.length > 0,
    mapRows: (rows) => rows[0]?.instructions ?? "",
  });
  return data;
}

export async function loadFolderWorkspaceId(
  folderPath: string,
): Promise<string> {
  const path = normalizeFolderPath(folderPath);
  if (!path) {
    return "";
  }

  const rows = await liveQueryClient.execute<{ workspace_id: string }>(
    `
      SELECT workspace_id
      FROM folders
      WHERE path = ?
        AND deleted_at IS NULL
      LIMIT 1
    `,
    [path],
  );
  return rows[0]?.workspace_id ?? "";
}

export function useFolderWorkspaceId(folderPath: string): string {
  const { data = "" } = useLiveQuery<{ workspace_id: string }, string>({
    sql: `
      SELECT workspace_id
      FROM folders
      WHERE path = ?
        AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [folderPath],
    enabled: folderPath.length > 0,
    mapRows: (rows) => rows[0]?.workspace_id ?? "",
  });
  return data;
}

async function folderNameTaken(path: string): Promise<boolean> {
  const rows = await liveQueryClient.execute<{ present: number }>(
    `
      SELECT 1 AS present
      FROM (
        SELECT path AS folder_path
        FROM folders
        WHERE deleted_at IS NULL
          AND path = ?
        UNION
        SELECT folder_path
        FROM folder_attachments
        WHERE deleted_at IS NULL
          AND folder_path = ?
        UNION
        SELECT folder_path
        FROM sessions
        WHERE deleted_at IS NULL
          AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
      )
      LIMIT 1
    `,
    [path, path, path, `${path}/%`, `${path}\\%`],
  );
  return rows.length > 0;
}

async function renameFolderOnDisk(oldPath: string, newPath: string) {
  const renamed = await fsSyncCommands.renameFolder(oldPath, newPath);
  if (renamed.status === "ok") {
    return;
  }
  if (!String(renamed.error).includes("folder_source_missing")) {
    throw new Error(renamed.error);
  }

  const created = await fsSyncCommands.createFolder(newPath);
  if (created.status === "error") {
    throw new Error(created.error);
  }
}

function requireNamedFolderPath(value: string): string {
  const normalized = normalizeFolderPath(value);
  if (!normalized) {
    throw new Error("invalid folder path");
  }
  return normalized;
}
