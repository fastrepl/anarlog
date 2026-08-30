import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { normalizeFolderPath } from "./folders";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

export async function ensureFolderCatalog(folderPath: string): Promise<string> {
  const path = requireNamedFolderPath(folderPath);
  await enqueueDatabaseWrite("folders", () =>
    executeTransaction(ensureFolderStatements(path)),
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
