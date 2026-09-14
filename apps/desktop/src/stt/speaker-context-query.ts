import { useLiveQuery } from "~/db";
import {
  EMPTY_SPEAKER_CONTEXT,
  parseSpeakerContext,
} from "~/stt/speaker-context";

export function useSpeakerContext(sessionId: string) {
  const { data = EMPTY_SPEAKER_CONTEXT } = useLiveQuery<
    { context: string | null },
    ReturnType<typeof parseSpeakerContext>
  >({
    sql: "SELECT json_extract(metadata_json, '$.speaker_context') AS context FROM sessions WHERE id = ? AND deleted_at IS NULL",
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => parseSpeakerContext(rows[0]?.context),
  });
  return data;
}
