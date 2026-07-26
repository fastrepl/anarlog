import { json2md } from "@hypr/editor/markdown";

import { liveQueryClient } from "~/db";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";

type SessionContentSqlRow = {
  id: string;
  owner_user_id: string;
  title: string;
  created_at: string;
  event_json: string;
  event_id: string;
  raw_note_id: string;
  raw_body: string;
  raw_body_format: string;
  enhanced_notes_json: string;
  transcripts_json: string;
  participants_json: string;
};

type EnhancedNoteJson = {
  id: string;
  title: string;
  body: string;
  body_format: string;
  template_id: string;
  sort_order: number;
};

type TranscriptJson = {
  id: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  memo: string;
  words_json: string;
  speaker_hints_json: string;
};

type ParticipantJson = {
  human_id: string;
  name: string;
  job_title: string;
};

export type SessionContentSnapshot = {
  sessionId: string;
  ownerUserId: string;
  title: string;
  createdAt: string;
  event: unknown;
  eventId: string | null;
  rawNoteId: string | null;
  rawContent: string;
  rawContentFormat: string;
  rawMarkdown: string;
  enhancedNotes: Array<{
    id: string;
    title: string;
    markdown: string;
    content: string;
    contentFormat: string;
    templateId: string;
    position: number;
  }>;
  transcripts: Array<{
    id: string;
    started_at: number;
    ended_at: number | null;
    memo: string;
    wordsJson: string;
    words: WordWithId[];
    speaker_hints: SpeakerHintWithId[];
  }>;
  participants: Array<{
    humanId: string;
    name: string;
    jobTitle: string;
  }>;
};

const SESSION_CONTENT_SQL = `
  SELECT
    session.id,
    session.owner_user_id,
    session.title,
    session.created_at,
    session.event_json,
    COALESCE(NULLIF(session.event_id, ''), NULLIF(session.external_event_id, ''), '') AS event_id,
    COALESCE(note.id, '') AS raw_note_id,
    COALESCE(note.body, '') AS raw_body,
    COALESCE(note.body_format, 'prosemirror_json') AS raw_body_format,
    COALESCE((
      SELECT json_group_array(json_object(
        'id', document.id,
        'title', document.title,
        'body', document.body,
        'body_format', document.body_format,
        'template_id', document.template_id,
        'sort_order', document.sort_order
      ))
      FROM session_documents AS document
      WHERE document.session_id = session.id
        AND document.kind IN ('summary', 'template_output')
        AND document.deleted_at IS NULL
    ), '[]') AS enhanced_notes_json,
    COALESCE((
      SELECT json_group_array(json_object(
        'id', transcript.id,
        'started_at_ms', transcript.started_at_ms,
        'ended_at_ms', transcript.ended_at_ms,
        'memo', transcript.memo,
        'words_json', transcript.words_json,
        'speaker_hints_json', transcript.speaker_hints_json
      ))
      FROM transcripts AS transcript
      WHERE transcript.session_id = session.id
        AND transcript.deleted_at IS NULL
    ), '[]') AS transcripts_json,
    COALESCE((
      SELECT json_group_array(json_object(
        'human_id', participant.human_id,
        'name', COALESCE(NULLIF(human.name, ''), participant.display_name),
        'job_title', COALESCE(human.job_title, '')
      ))
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id
        AND human.deleted_at IS NULL
      WHERE participant.session_id = session.id
        AND participant.human_id <> ''
        AND participant.source <> 'excluded'
        AND participant.deleted_at IS NULL
    ), '[]') AS participants_json
  FROM sessions AS session
  LEFT JOIN session_documents AS note
    ON note.id = COALESCE(
      (
        SELECT canonical.id
        FROM session_documents AS canonical
        WHERE canonical.id = session.id
          AND canonical.session_id = session.id
          AND canonical.kind = 'note'
          AND canonical.deleted_at IS NULL
        LIMIT 1
      ),
      (
        SELECT fallback.id
        FROM session_documents AS fallback
        WHERE fallback.session_id = session.id
          AND fallback.kind = 'note'
          AND fallback.deleted_at IS NULL
        ORDER BY fallback.created_at, fallback.id
        LIMIT 1
      )
    )
  WHERE session.id = ? AND session.deleted_at IS NULL
  LIMIT 1
`;

export async function loadSessionContentSnapshot(
  sessionId: string,
): Promise<SessionContentSnapshot | null> {
  if (!sessionId) return null;
  const rows = await liveQueryClient.execute<SessionContentSqlRow>(
    SESSION_CONTENT_SQL,
    [sessionId],
  );
  const row = rows[0];
  return row ? mapSessionContentRow(row) : null;
}

export async function loadActiveSessionIds(): Promise<string[]> {
  const rows = await liveQueryClient.execute<{ id: string }>(
    `
      SELECT id
      FROM sessions
      WHERE deleted_at IS NULL
      ORDER BY created_at DESC, id
    `,
  );
  return rows.map((row) => row.id);
}

function mapSessionContentRow(
  row: SessionContentSqlRow,
): SessionContentSnapshot {
  const enhancedNotes = parseJsonArray<EnhancedNoteJson>(
    row.enhanced_notes_json,
  )
    .map((note) => ({
      id: note.id,
      title: note.title,
      markdown: bodyToMarkdown(note.body, note.body_format),
      content: note.body,
      contentFormat: note.body_format,
      templateId: note.template_id,
      position: Number(note.sort_order),
    }))
    .sort(
      (left, right) =>
        left.position - right.position || left.id.localeCompare(right.id),
    );

  const transcripts = parseJsonArray<TranscriptJson>(row.transcripts_json)
    .map((transcript) => ({
      id: transcript.id,
      started_at: Number(transcript.started_at_ms),
      ended_at:
        transcript.ended_at_ms == null ? null : Number(transcript.ended_at_ms),
      memo: transcript.memo,
      wordsJson: transcript.words_json,
      words: parseJsonArray<WordWithId>(transcript.words_json),
      speaker_hints: parseJsonArray<SpeakerHintWithId>(
        transcript.speaker_hints_json,
      ),
    }))
    .sort(
      (left, right) =>
        left.started_at - right.started_at || left.id.localeCompare(right.id),
    );

  const participants = parseJsonArray<ParticipantJson>(row.participants_json)
    .map((participant) => ({
      humanId: participant.human_id,
      name: participant.name,
      jobTitle: participant.job_title,
    }))
    .sort(
      (left, right) =>
        left.name.localeCompare(right.name) ||
        left.humanId.localeCompare(right.humanId),
    );

  return {
    sessionId: row.id,
    ownerUserId: row.owner_user_id,
    title: row.title,
    createdAt: row.created_at,
    event: parseJson(row.event_json),
    eventId: row.event_id || null,
    rawNoteId: row.raw_note_id || null,
    rawContent: row.raw_body,
    rawContentFormat: row.raw_body_format,
    rawMarkdown: bodyToMarkdown(row.raw_body, row.raw_body_format),
    enhancedNotes,
    transcripts,
    participants,
  };
}

function bodyToMarkdown(body: string, format: string): string {
  if (!body || format === "markdown") return body;
  try {
    return json2md(JSON.parse(body));
  } catch {
    return body;
  }
}

function parseJson(value: string): unknown {
  if (!value) return null;
  try {
    return JSON.parse(value) as unknown;
  } catch {
    return null;
  }
}

function parseJsonArray<T>(value: string): T[] {
  try {
    const parsed = JSON.parse(value) as unknown;
    return Array.isArray(parsed) ? (parsed as T[]) : [];
  } catch {
    return [];
  }
}
