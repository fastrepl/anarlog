import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { ancestorFolderPaths, normalizeFolderPath } from "./folders";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

export async function ensureFolderCatalog(folderPath: string): Promise<string> {
  const path = requireNamedFolderPath(folderPath);
  await enqueueDatabaseWrite("folders", () =>
    executeTransaction(
      ancestorFolderPaths(path).flatMap((ancestor) =>
        ensureFolderStatements(ancestor),
      ),
    ),
  );
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

  await enqueueDatabaseWrite("folders", () =>
    executeTransaction([
      {
        sql: `
          UPDATE folders
          SET
            path = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            deleted_at = NULL
          WHERE path = ?
            AND deleted_at IS NULL
        `,
        params: [newPath, oldPath],
      },
      ...ensureFolderStatements(newPath),
      {
        sql: `
          UPDATE folder_attachments
          SET
            folder_path = CASE
              WHEN folder_path = ? THEN ?
              ELSE ? || substr(folder_path, length(?) + 1)
            END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE deleted_at IS NULL
            AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
        `,
        params: nestedFolderRewriteParams(oldPath, newPath),
      },
      {
        sql: `
          UPDATE sessions
          SET
            folder_path = CASE
              WHEN folder_path = ? THEN ?
              ELSE ? || substr(folder_path, length(?) + 1)
            END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE deleted_at IS NULL
            AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
        `,
        params: nestedFolderRewriteParams(oldPath, newPath),
      },
    ]),
  );

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

  await enqueueDatabaseWrite("folders", () =>
    executeTransaction([
      {
        sql: `
          UPDATE folders
          SET
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE deleted_at IS NULL
            AND (path = ? OR path LIKE ? OR path LIKE ?)
        `,
        params: [path, `${path}/%`, `${path}\\%`],
      },
      {
        sql: `
          UPDATE folder_attachments
          SET
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE deleted_at IS NULL
            AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
        `,
        params: [path, `${path}/%`, `${path}\\%`],
      },
      {
        sql: `
          UPDATE sessions
          SET
            folder_path = '',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE deleted_at IS NULL
            AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
        `,
        params: [path, `${path}/%`, `${path}\\%`],
      },
    ]),
  );

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
  await enqueueDatabaseWrite("folders", () =>
    executeTransaction([
      {
        sql: `
          UPDATE folders
          SET
            instructions = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE path = ?
            AND deleted_at IS NULL
        `,
        params: [instructions, path],
      },
    ]),
  );
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

function ensureFolderStatements(path: string) {
  const metadataId = id();
  return [
    {
      sql: `
        UPDATE folders
        SET
          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
          deleted_at = NULL
        WHERE id = (
          SELECT id
          FROM folders
          WHERE path = ?
          ORDER BY deleted_at IS NULL DESC,
            updated_at DESC,
            id
          LIMIT 1
        )
      `,
      params: [path],
    },
    {
      sql: `
        INSERT INTO folders (
          id,
          workspace_id,
          path
        )
        SELECT
          ?,
          COALESCE((
            SELECT session.workspace_id
            FROM sessions AS session
            WHERE session.deleted_at IS NULL
              AND (session.folder_path = ? OR session.folder_path LIKE ?)
            ORDER BY session.updated_at DESC, session.id
            LIMIT 1
          ), ''),
          ?
        WHERE NOT EXISTS (
          SELECT 1
          FROM folders
          WHERE path = ?
            AND deleted_at IS NULL
        )
      `,
      params: [metadataId, path, `${path}/%`, path, path],
    },
  ];
}

function nestedFolderRewriteParams(oldPath: string, newPath: string) {
  return [
    oldPath,
    newPath,
    newPath,
    oldPath,
    oldPath,
    `${oldPath}/%`,
    `${oldPath}\\%`,
  ];
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
