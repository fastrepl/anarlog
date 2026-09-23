import type {
  RenderTranscriptRequest,
  SpeakerContext,
} from "@anlg/plugin-transcription";

import { liveQueryClient } from "~/db";
import type { RenderLabelContext } from "~/stt/live-segment";
import { parseSpeakerContext } from "~/stt/speaker-context";

type MeetingFloatSqlRow = {
  row_kind: "session" | "participant" | "human";
  session_id: string;
  title: string;
  owner_user_id: string;
  human_id: string;
  human_name: string;
  speaker_context: string | null;
  live_started_at_ms: number | null;
};

export type MeetingFloatData = {
  sessions: Record<
    string,
    {
      title: string;
      ownerUserId: string;
      participantHumanIds: string[];
      speakerContext: SpeakerContext;
      startedAtMs: number;
    }
  >;
  humanNames: Record<string, string>;
};

const MEETING_FLOAT_SQL = `
  SELECT
    'session' AS row_kind,
    session.id AS session_id,
    session.title,
    session.owner_user_id,
    '' AS human_id,
    '' AS human_name,
    json_extract(session.metadata_json, '$.speaker_context') AS speaker_context,
    (
      SELECT t.started_at_ms FROM transcripts AS t
      WHERE t.session_id = session.id
      ORDER BY t.started_at_ms DESC LIMIT 1
    ) AS live_started_at_ms
  FROM sessions AS session
  WHERE session.deleted_at IS NULL

  UNION ALL

  SELECT
    'participant' AS row_kind,
    participant.session_id,
    '' AS title,
    session.owner_user_id,
    participant.human_id,
    COALESCE(NULLIF(human.name, ''), participant.display_name) AS human_name,
    '' AS speaker_context,
    NULL AS live_started_at_ms
  FROM session_participants AS participant
  INNER JOIN sessions AS session
    ON session.id = participant.session_id
    AND session.deleted_at IS NULL
  LEFT JOIN humans AS human
    ON human.id = participant.human_id
    AND human.deleted_at IS NULL
  WHERE participant.human_id <> ''
    AND participant.source <> 'excluded'
    AND participant.deleted_at IS NULL

  UNION ALL

  SELECT
    'human' AS row_kind,
    '' AS session_id,
    '' AS title,
    '' AS owner_user_id,
    human.id AS human_id,
    human.name AS human_name,
    '' AS speaker_context,
    NULL AS live_started_at_ms
  FROM humans AS human
  WHERE human.id <> '' AND human.deleted_at IS NULL

  ORDER BY row_kind, session_id, human_id
`;

export async function loadMeetingFloatData(): Promise<MeetingFloatData> {
  return mapMeetingFloatRows(
    await liveQueryClient.execute<MeetingFloatSqlRow>(MEETING_FLOAT_SQL),
  );
}

export async function subscribeMeetingFloatData(
  onData: (data: MeetingFloatData) => void,
  onError: (error: string) => void,
): Promise<() => Promise<void>> {
  return liveQueryClient.subscribe<MeetingFloatSqlRow>(MEETING_FLOAT_SQL, [], {
    onData: (rows) => onData(mapMeetingFloatRows(rows)),
    onError,
  });
}

export function createMeetingFloatLabelContext(
  data: MeetingFloatData,
  sessionId: string,
): RenderLabelContext {
  const session = data.sessions[sessionId];
  return {
    getSelfHumanId: () => session?.ownerUserId || undefined,
    getHumanName: (humanId) => data.humanNames[humanId] || undefined,
    getParticipantHumanIds: () => session?.participantHumanIds ?? [],
  };
}

function mapMeetingFloatRows(rows: MeetingFloatSqlRow[]): MeetingFloatData {
  const sessions: MeetingFloatData["sessions"] = {};
  const humanNames: MeetingFloatData["humanNames"] = {};

  for (const row of rows) {
    if (row.row_kind === "session") {
      sessions[row.session_id] = {
        title: row.title,
        ownerUserId: row.owner_user_id,
        participantHumanIds:
          sessions[row.session_id]?.participantHumanIds ?? [],
        speakerContext: parseSpeakerContext(row.speaker_context),
        startedAtMs: row.live_started_at_ms ?? 0,
      };
      continue;
    }

    if (row.human_id && row.human_name) {
      humanNames[row.human_id] = row.human_name;
    }
    if (row.row_kind !== "participant") {
      continue;
    }

    const session = sessions[row.session_id] ?? {
      title: "",
      ownerUserId: row.owner_user_id,
      participantHumanIds: [],
      speakerContext: parseSpeakerContext(null),
      startedAtMs: 0,
    };
    if (!session.participantHumanIds.includes(row.human_id)) {
      session.participantHumanIds.push(row.human_id);
    }
    sessions[row.session_id] = session;
  }

  return { sessions, humanNames };
}

export function createMeetingFloatRenderRequest(
  data: MeetingFloatData,
  sessionId: string,
  startedAt: number,
): RenderTranscriptRequest | null {
  const session = data.sessions[sessionId];
  if (!session || session.speakerContext.intervals.length === 0) {
    return null;
  }

  const humanIds = [
    ...new Set([session.ownerUserId, ...session.participantHumanIds]),
  ].filter(Boolean);

  return {
    transcripts: [{ started_at: startedAt, words: [], assignments: [] }],
    participant_human_ids: session.participantHumanIds,
    self_human_id: session.ownerUserId || null,
    humans: humanIds.map((humanId) => ({
      human_id: humanId,
      name: data.humanNames[humanId] ?? "",
    })),
    speaker_context: session.speakerContext,
  };
}
