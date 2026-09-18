import { useLiveQuery } from "~/db";
import { WELCOME_NOTE_TRACKING_ID } from "~/onboarding/welcome-note.constants";

export type ActivityRecord = {
  session_id: string;
  started_at_ms: number;
  created_at: string;
  duration_ms: number;
  is_demo?: number;
};

export const ACTIVITY_SQL = `
  SELECT
    transcript.session_id,
    transcript.started_at_ms,
    transcript.created_at,
    CASE WHEN json_valid(session.event_json)
      THEN COALESCE(json_extract(session.event_json, '$.tracking_id') = '${WELCOME_NOTE_TRACKING_ID}', 0)
      ELSE 0 END AS is_demo,
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
    AND CASE WHEN word.type = 'object'
      THEN LENGTH(TRIM(COALESCE(json_extract(word.value, '$.text'), ''))) > 0
      ELSE 0 END
  GROUP BY transcript.id
`;

export function useActivity() {
  return useLiveQuery<ActivityRecord, ActivityRecord[]>({
    sql: ACTIVITY_SQL,
    params: [],
  });
}
