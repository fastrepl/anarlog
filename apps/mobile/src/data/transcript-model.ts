export const SESSION_TRANSCRIPTS_SQL = `SELECT transcript.id, transcript.source, transcript.provider, transcript.started_at_ms, transcript.owner_user_id, transcript.words_json, transcript.speaker_hints_json,
      transcript.metadata_json,
      (SELECT json_extract(session.metadata_json, '$.speaker_context') FROM sessions AS session WHERE session.id = transcript.session_id) AS speaker_context_json,
      COALESCE((SELECT json_group_array(json(ordered_delta.delta_json)) FROM (
        SELECT delta.delta_json FROM transcript_live_deltas AS delta
        WHERE delta.transcript_id = transcript.id ORDER BY delta.sequence
      ) AS ordered_delta), '[]') AS pending_deltas_json
      FROM transcripts AS transcript WHERE transcript.session_id = ? AND transcript.deleted_at IS NULL
      ORDER BY transcript.started_at_ms, transcript.created_at, transcript.id`;

export const SESSION_SPEAKERS_SQL = `SELECT id, name FROM humans WHERE workspace_id = (SELECT workspace_id FROM sessions WHERE id = ?) AND name <> ''`;

export const SESSION_HAS_TRANSCRIPT_SQL = `SELECT EXISTS (
  SELECT 1 FROM transcripts WHERE session_id = ? AND deleted_at IS NULL
) AS has_transcript`;

export type TranscriptRow = {
  id: string;
  source?: string;
  provider?: string;
  started_at_ms: number;
  owner_user_id: string;
  speaker_context_json: string | null;
  words_json: string;
  speaker_hints_json: string;
  pending_deltas_json: string;
  metadata_json?: string;
};

export type TranscriptSegment = {
  id: string;
  text: string;
  speaker: string;
  startMs: number;
  endMs: number;
  wordCount: number;
  originalText?: string;
  revisionCount?: number;
};

type TranscriptEdit = {
  text: string;
  revisions: { text: string; edited_at: string }[];
};

export function applyTranscriptEdits<T extends TranscriptSegment>(
  segments: T[],
  metadataJson: string | null | undefined,
): T[] {
  if (!metadataJson) return segments;
  let edits: Record<string, TranscriptEdit>;
  try {
    const metadata: unknown = JSON.parse(metadataJson);
    if (!metadata || typeof metadata !== "object") return segments;
    const candidate = (metadata as { transcript_edits?: unknown })
      .transcript_edits;
    if (!candidate || typeof candidate !== "object" || Array.isArray(candidate))
      return segments;
    edits = candidate as Record<string, TranscriptEdit>;
  } catch {
    return segments;
  }
  return segments.map((segment) => {
    if (!Object.prototype.hasOwnProperty.call(edits, segment.id))
      return segment;
    const edit = edits[segment.id];
    if (
      !edit ||
      typeof edit !== "object" ||
      typeof edit.text !== "string" ||
      edit.text === segment.text
    )
      return segment;
    return {
      ...segment,
      text: edit.text,
      originalText: segment.text,
      revisionCount: Array.isArray(edit.revisions)
        ? edit.revisions.filter(
            (revision) =>
              revision &&
              typeof revision.text === "string" &&
              typeof revision.edited_at === "string",
          ).length
        : 0,
    };
  });
}

export function transcriptEditPatch(
  segmentId: string,
  originalText: string,
  nextText: string,
  editedAt: string,
  previous?: TranscriptEdit,
) {
  return {
    transcript_edits: {
      [segmentId]: {
        text: nextText,
        revisions: [
          ...(previous?.revisions ?? []),
          { text: previous?.text ?? originalText, edited_at: editedAt },
        ],
      },
    },
  };
}

export function transcriptEditStatements(
  transcriptId: string,
  sessionId: string,
  expectedMetadataJson: string,
  patchJson: string,
  updatedAt: string,
) {
  return [
    {
      sql: `UPDATE transcripts
       SET metadata_json = json_patch(
         CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,
         json(?)
       ), updated_at = ?
       WHERE id = ? AND session_id = ? AND deleted_at IS NULL
         AND metadata_json = ?`,
      params: [
        patchJson,
        updatedAt,
        transcriptId,
        sessionId,
        expectedMetadataJson,
      ],
    },
    {
      sql: `UPDATE session_documents
       SET generation_metadata_json = json_set(
         CASE WHEN json_valid(generation_metadata_json)
           AND json_type(generation_metadata_json) = 'object'
           THEN generation_metadata_json ELSE '{}' END,
         '$.transcript_stale', json('true')
       ), updated_at = ?
       WHERE changes() > 0 AND session_id = ? AND kind IN ('summary', 'template_output')
         AND deleted_at IS NULL`,
      params: [updatedAt, sessionId],
    },
  ];
}

export function sharedMicrophoneContextStatement(
  sessionId: string,
  startedAtMs: number,
  durationMs: number,
  updatedAt: string,
) {
  return {
    sql: `UPDATE sessions SET metadata_json = json_set(
      metadata_json, '$.speaker_context', json_insert(
        COALESCE(json_extract(metadata_json, '$.speaker_context'), '{"intervals":[]}'),
        '$.intervals[#]', json(?)
      )
    ), updated_at = ? WHERE id = ? AND deleted_at IS NULL`,
    params: [
      JSON.stringify({
        start_ms: startedAtMs,
        end_ms: startedAtMs + Math.max(1, Math.ceil(durationMs)),
        active_call: false,
        calendar_call: false,
        mic_isolated: null,
        shared_microphone: true,
        title: "",
        self_names: [],
        participants: [],
      }),
      updatedAt,
      sessionId,
    ],
  };
}
