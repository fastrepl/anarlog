import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { normalizeFolderPath } from "./folders";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

const SHA256_PATTERN = /^[0-9a-f]{64}$/;

export type FolderMaterialRecord = {
  id: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  relativePath: string;
};

type FolderMaterialSqlRow = {
  id: string;
  filename: string;
  content_type: string;
  size_bytes: number;
  relative_path: string;
};

const EMPTY_FOLDER_MATERIALS: FolderMaterialRecord[] = [];

const FOLDER_MATERIAL_COLUMNS = `
  SELECT id, filename, content_type, size_bytes, relative_path
  FROM folder_attachments
`;

function mapFolderMaterial(row: FolderMaterialSqlRow): FolderMaterialRecord {
  return {
    id: row.id,
    filename: row.filename,
    contentType: row.content_type,
    sizeBytes: row.size_bytes,
    relativePath: row.relative_path,
  };
}

export function useFolderMaterials(folderPath: string): FolderMaterialRecord[] {
  const { data = EMPTY_FOLDER_MATERIALS } = useLiveQuery<
    FolderMaterialSqlRow,
    FolderMaterialRecord[]
  >({
    sql: `
      ${FOLDER_MATERIAL_COLUMNS}
      WHERE folder_path = ?
        AND deleted_at IS NULL
      ORDER BY filename, id
    `,
    params: [folderPath],
    enabled: folderPath.length > 0,
    mapRows: (rows) => rows.map(mapFolderMaterial),
  });
  return data;
}

export async function loadFolderMaterials(
  folderPath: string,
): Promise<FolderMaterialRecord[]> {
  const namedPath = requireNamedFolderPath(folderPath);
  const rows = await liveQueryClient.execute<FolderMaterialSqlRow>(
    `
      ${FOLDER_MATERIAL_COLUMNS}
      WHERE folder_path = ?
        AND deleted_at IS NULL
      ORDER BY filename, id
    `,
    [namedPath],
  );
  return rows.map(mapFolderMaterial);
}

export async function loadFolderMaterial(
  folderPath: string,
  attachmentId: string,
): Promise<FolderMaterialRecord | null> {
  const namedPath = requireNamedFolderPath(folderPath);
  const materialId = requireText(attachmentId, "folder material ID", 512);
  const rows = await liveQueryClient.execute<FolderMaterialSqlRow>(
    `
      ${FOLDER_MATERIAL_COLUMNS}
      WHERE folder_path = ?
        AND id = ?
        AND deleted_at IS NULL
      LIMIT 1
    `,
    [namedPath, materialId],
  );
  const row = rows[0];
  return row ? mapFolderMaterial(row) : null;
}

export async function catalogLocalFolderMaterial(input: {
  folderPath: string;
  attachmentId: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
}): Promise<void> {
  const folderPath = requireNamedFolderPath(input.folderPath);
  const attachmentId = requireBasename(input.attachmentId, "attachment ID");
  const filename = requireBasename(input.filename, "attachment filename");
  const contentType = requireText(
    input.contentType,
    "attachment content type",
    512,
    true,
  );
  if (!Number.isSafeInteger(input.sizeBytes) || input.sizeBytes < 0) {
    throw new Error("invalid attachment size");
  }
  if (!SHA256_PATTERN.test(input.sha256)) {
    throw new Error("invalid attachment checksum");
  }

  const relativePath = `materials/${attachmentId}`;
  const metadataId = id();
  const results = await enqueueDatabaseWrite(`folder:${folderPath}`, () =>
    executeTransaction([
      {
        sql: `
          UPDATE folder_attachments
          SET
            filename = ?,
            content_type = ?,
            size_bytes = ?,
            sha256 = ?,
            source_type = 'folder_material',
            source_id = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            deleted_at = NULL
          WHERE id = (
            SELECT id
            FROM folder_attachments
            WHERE folder_path = ?
              AND relative_path = ?
            ORDER BY deleted_at IS NULL DESC,
              updated_at DESC,
              id
            LIMIT 1
          )
        `,
        params: [
          filename,
          contentType,
          input.sizeBytes,
          input.sha256,
          attachmentId,
          folderPath,
          relativePath,
        ],
      },
      {
        sql: `
          INSERT INTO folder_attachments (
            id,
            workspace_id,
            folder_path,
            filename,
            relative_path,
            content_type,
            size_bytes,
            sha256,
            storage_kind,
            cloud_object_key,
            source_type,
            source_id,
            metadata_json
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
            ?,
            ?,
            ?,
            ?,
            ?,
            ?,
            'local_file',
            '',
            'folder_material',
            ?,
            '{}'
          WHERE NOT EXISTS (
            SELECT 1
            FROM folder_attachments
            WHERE folder_path = ?
              AND relative_path = ?
              AND deleted_at IS NULL
          )
        `,
        params: [
          metadataId,
          folderPath,
          `${folderPath}/%`,
          folderPath,
          filename,
          relativePath,
          contentType,
          input.sizeBytes,
          input.sha256,
          attachmentId,
          folderPath,
          relativePath,
        ],
      },
    ]),
  );

  if ((results[0] ?? 0) + (results[1] ?? 0) !== 1) {
    throw new Error("folder material is unavailable");
  }
}

export async function deleteLocalFolderMaterial(input: {
  folderPath: string;
  attachmentId: string;
}): Promise<void> {
  const folderPath = requireNamedFolderPath(input.folderPath);
  const attachmentId = requireBasename(input.attachmentId, "attachment ID");
  const relativePath = `materials/${attachmentId}`;

  await enqueueDatabaseWrite(`folder:${folderPath}`, async () => {
    const [updated = 0] = await executeTransaction([
      {
        sql: `
          UPDATE folder_attachments
          SET
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE folder_path = ?
            AND relative_path = ?
            AND deleted_at IS NULL
        `,
        params: [folderPath, relativePath],
      },
    ]);
    if (updated !== 1) {
      throw new Error("folder material is unavailable");
    }

    const result = await fsSyncCommands.folderAttachmentRemove(
      folderPath,
      attachmentId,
    );
    if (result.status === "error") {
      throw new Error(result.error);
    }
  });
}

export function diskAttachmentId(relativePath: string): string {
  const parts = relativePath.split("/");
  return parts[parts.length - 1] ?? relativePath;
}

function requireNamedFolderPath(value: string): string {
  const normalized = normalizeFolderPath(value);
  if (!normalized) {
    throw new Error("invalid folder path");
  }
  return normalized;
}

function requireBasename(value: unknown, label: string) {
  const basename = requireText(value, label, 1024);
  if (
    basename === "." ||
    basename === ".." ||
    basename.includes("/") ||
    basename.includes("\\") ||
    basename.includes("\0")
  ) {
    throw new Error(`invalid ${label}`);
  }
  return basename;
}

function requireText(
  value: unknown,
  label: string,
  maxLength: number,
  allowEmpty = false,
) {
  if (
    typeof value !== "string" ||
    (!allowEmpty && value.length === 0) ||
    value.length > maxLength ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/.test(value)
  ) {
    throw new Error(`invalid ${label}`);
  }
  return value;
}
