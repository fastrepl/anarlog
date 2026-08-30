import { collectFolderPaths, folderDisplayName } from "../folders";
import type { SessionSummaryRecord } from "./types";

import { liveQueryClient, useLiveQuery } from "~/db";

type FolderPathSqlRow = {
  folder_path: string;
};

const EMPTY_FOLDER_PATHS: string[] = [];

export const FOLDER_PATHS_SQL = `
  SELECT folder_path
  FROM (
    SELECT folder_path
    FROM sessions
    WHERE deleted_at IS NULL
      AND folder_path != ''
    UNION
    SELECT folder_path
    FROM folder_attachments
    WHERE deleted_at IS NULL
      AND folder_path != ''
    UNION
    SELECT path AS folder_path
    FROM folders
    WHERE deleted_at IS NULL
      AND path != ''
  )
`;

export function useFolderPaths(): string[] {
  const { data = EMPTY_FOLDER_PATHS } = useLiveQuery<
    FolderPathSqlRow,
    string[]
  >({
    sql: FOLDER_PATHS_SQL,
    mapRows: (rows) => collectFolderPaths(rows.map((row) => row.folder_path)),
  });
  return data;
}

type FolderSessionSqlRow = {
  id: string;
  title: string;
  created_at: string;
  event_json: string;
  folder_path: string;
};

export type FolderSessionSummary = SessionSummaryRecord & {
  event_json: string;
};

function folderSessionFilterSql(folderFilter: string): {
  sql: string;
  params: string[];
} {
  if (folderFilter === "") {
    return { sql: "folder_path = ''", params: [] };
  }

  return {
    sql: "(folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)",
    params: [folderFilter, `${folderFilter}/%`, `${folderFilter}\\%`],
  };
}

export async function loadSessionSummariesByFolder(
  folderFilter: string,
): Promise<FolderSessionSummary[]> {
  const { sql, params } = folderSessionFilterSql(folderFilter);
  const rows = await liveQueryClient.execute<FolderSessionSqlRow>(
    `
      SELECT id, title, created_at, event_json, folder_path
      FROM sessions
      WHERE deleted_at IS NULL
        AND ${sql}
      ORDER BY created_at DESC
    `,
    params,
  );

  return rows
    .filter((row) => folderDisplayName(row.folder_path) === folderFilter)
    .map((row) => ({
      id: row.id,
      title: row.title,
      created_at: row.created_at,
      event_json: row.event_json,
    }));
}
