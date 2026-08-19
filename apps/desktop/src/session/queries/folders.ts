import { collectFolderPaths } from "../folders";

import { useLiveQuery } from "~/db";

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
