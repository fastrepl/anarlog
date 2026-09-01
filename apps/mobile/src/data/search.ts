import {
  mapTimelineRows,
  type TimelineRow,
  type TimelineSession,
} from "@/data/timeline";
import { useLiveQuery } from "@/db";

const SEARCH_SQL = `
SELECT DISTINCT
  sessions.id,
  sessions.title,
  sessions.created_at,
  sessions.event_json,
  sessions.folder_path,
  COALESCE((
    SELECT json_group_array(name)
    FROM (
      SELECT DISTINCT tags.name AS name
      FROM session_tags
      JOIN tags ON tags.id = session_tags.tag_id
      WHERE session_tags.session_id = sessions.id
        AND session_tags.deleted_at IS NULL
        AND tags.deleted_at IS NULL
        AND trim(tags.name) <> ''
      ORDER BY tags.name COLLATE NOCASE
    )
  ), '[]') AS tags_json
FROM sessions
LEFT JOIN session_documents AS note
  ON note.session_id = sessions.id AND note.kind = 'note' AND note.deleted_at IS NULL
WHERE sessions.deleted_at IS NULL
  AND (sessions.title LIKE ? ESCAPE '\\' OR note.body LIKE ? ESCAPE '\\')
ORDER BY sessions.created_at DESC
LIMIT 50
`;

function escapeLike(term: string): string {
  return term.replace(/[\\%_]/g, (char) => `\\${char}`);
}

export function useSessionSearch(query: string): {
  results: TimelineSession[];
  isLoading: boolean;
} {
  const term = query.trim();
  const pattern = `%${escapeLike(term)}%`;
  const { data, isLoading } = useLiveQuery<TimelineRow, TimelineSession[]>({
    sql: SEARCH_SQL,
    params: [pattern, pattern],
    enabled: term !== "",
    mapRows: mapTimelineRows,
  });
  return { results: term === "" ? [] : (data ?? []), isLoading };
}
