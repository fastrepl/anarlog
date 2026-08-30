import { collectFolderPaths, folderDisplayName } from "../folders";
import type { SessionSummaryRecord } from "./types";

import { liveQueryClient, useLiveQuery } from "~/db";

type FolderPathSqlRow = {
  folder_path: string;
};

const EMPTY_FOLDER_PATHS: string[] = [];

export function useFolderPaths(): string[] {
  const { data = EMPTY_FOLDER_PATHS } = useLiveQuery<
    FolderPathSqlRow,
    string[]
  >({
    sql: `
      SELECT DISTINCT folder_path
      FROM sessions
      WHERE deleted_at IS NULL
        AND folder_path != ''
      ORDER BY folder_path
    `,
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
