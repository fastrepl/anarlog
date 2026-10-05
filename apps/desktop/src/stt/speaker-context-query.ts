import { liveQueryClient, useLiveQuery } from "~/db";
import {
  EMPTY_SPEAKER_CONTEXT,
  parseSpeakerContext,
} from "~/stt/speaker-context";

type SpeakerContextSqlRow = { context: string | null };

const SPEAKER_CONTEXT_SQL =
  "SELECT json_extract(metadata_json, '$.speaker_context') AS context FROM sessions WHERE id = ? AND deleted_at IS NULL";

export async function getSpeakerContext(sessionId: string) {
  if (!sessionId) {
    return EMPTY_SPEAKER_CONTEXT;
  }

  const rows = await liveQueryClient.execute<SpeakerContextSqlRow>(
    SPEAKER_CONTEXT_SQL,
    [sessionId],
  );
  return parseSpeakerContext(rows[0]?.context);
}

export function useSpeakerContext(sessionId: string) {
  const { data = EMPTY_SPEAKER_CONTEXT } = useLiveQuery<
    SpeakerContextSqlRow,
    ReturnType<typeof parseSpeakerContext>
  >({
    sql: SPEAKER_CONTEXT_SQL,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => parseSpeakerContext(rows[0]?.context),
  });
  return data;
}
