import { useAuth } from "~/auth";
import { useLiveQuery } from "~/db";
import { DEFAULT_USER_ID } from "~/shared/utils";

export type ActivityRecord = {
  session_id: string;
  started_at_ms: number;
  created_at: string;
  duration_ms: number;
};

export const ACTIVITY_SQL = `
  SELECT
    transcript.session_id,
    transcript.started_at_ms,
    transcript.created_at,
    MAX(CASE
      WHEN word.type = 'object'
        AND json_type(word.value, '$.end_ms') IN ('integer', 'real')
      THEN MAX(0, json_extract(word.value, '$.end_ms'))
      ELSE 0
    END) AS duration_ms
  FROM transcripts AS transcript
  JOIN sessions AS session ON session.id = transcript.session_id
  JOIN json_each(CASE
    WHEN json_valid(transcript.words_json) THEN
      CASE WHEN json_type(transcript.words_json) = 'array'
        THEN transcript.words_json ELSE '[]' END
    ELSE '[]'
  END) AS word
  WHERE session.deleted_at IS NULL
    AND transcript.deleted_at IS NULL
    AND COALESCE(session.owner_user_id, '') IN (?, '', '${DEFAULT_USER_ID}')
    AND CASE WHEN word.type = 'object'
      THEN LENGTH(TRIM(COALESCE(json_extract(word.value, '$.text'), ''))) > 0
      ELSE 0 END
  GROUP BY transcript.id
`;

export function useActivity() {
  const auth = useAuth();
  return useLiveQuery<ActivityRecord, ActivityRecord[]>({
    sql: ACTIVITY_SQL,
    params: [auth.session?.user.id ?? DEFAULT_USER_ID],
  });
}
