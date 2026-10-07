import { z } from "zod";

import { json2md } from "@anlg/editor/markdown";

import { liveQueryClient } from "~/db";
import type { AppliedTemplateSnapshot } from "~/session/queries/types";
import {
  parseSessionSourceApps,
  type SessionSourceApp,
} from "~/session/source-apps";
import { extractBodyHeadingsKey } from "~/session/title-content";
import type { SpeakerHintWithId, WordWithId } from "~/stt/types";

type SessionContentSqlRow = {
  id: string;
  owner_user_id: string;
  owner_email: string | null;
  title: string;
  created_at: string;
  event_json: string;
  source_apps_json: string;
  event_id: string;
  raw_note_id: string;
  raw_template_id: string;
  raw_updated_at: string;
  raw_metadata_json: string;
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
  email?: string;
  job_title: string;
};

export type SessionContentSnapshot = {
  sessionId: string;
  ownerUserId: string;
  ownerEmail?: string | null;
  title: string;
  createdAt: string;
  event: unknown;
  sourceApps: SessionSourceApp[];
  eventId: string | null;
  rawNoteId: string | null;
  rawTemplateId: string;
  rawUpdatedAt: string;
  rawHeadingsUpdatedAt: string;
  rawAppliedTemplate: AppliedTemplateSnapshot | null;
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
    speakerHintsJson: string;
    words: WordWithId[];
    speaker_hints: SpeakerHintWithId[];
  }>;
  participants: Array<{
    humanId: string;
    name: string;
    email?: string;
    jobTitle: string;
  }>;
};

const SESSION_CONTENT_SQL = `
  SELECT
    session.id,
    session.owner_user_id,
    (
      SELECT NULLIF(lower(self_human.email), '')
      FROM humans AS self_human
      WHERE self_human.id = session.owner_user_id
        AND self_human.deleted_at IS NULL
    ) AS owner_email,
    session.title,
    session.created_at,
    session.event_json,
    session.source_apps_json,
    COALESCE(NULLIF(session.event_id, ''), NULLIF(session.external_event_id, ''), '') AS event_id,
    COALESCE(note.id, '') AS raw_note_id,
    COALESCE(note.template_id, '') AS raw_template_id,
    COALESCE(note.updated_at, '') AS raw_updated_at,
    COALESCE(note.generation_metadata_json, '{}') AS raw_metadata_json,
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
        'words_json', CASE WHEN ? THEN transcript.words_json ELSE '[]' END,
        'speaker_hints_json', CASE WHEN ? THEN transcript.speaker_hints_json ELSE '[]' END
      ))
      FROM transcripts AS transcript
      WHERE transcript.session_id = session.id
        AND transcript.deleted_at IS NULL
    ), '[]') AS transcripts_json,
    COALESCE((
      SELECT json_group_array(json_object(
        'human_id', participant.human_id,
        'name', COALESCE(NULLIF(human.name, ''), participant.display_name),
        'email', COALESCE(NULLIF(human.email, ''), participant.email),
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
        AND (
          participant.human_id = session.owner_user_id
          OR NULLIF(lower(COALESCE(NULLIF(human.email, ''), participant.email)), '') IS NULL
          OR NOT EXISTS (
            SELECT 1
            FROM humans AS self_human
            WHERE self_human.id = session.owner_user_id
              AND self_human.deleted_at IS NULL
              AND NULLIF(lower(self_human.email), '') IS NOT NULL
              AND lower(self_human.email) = lower(COALESCE(NULLIF(human.email, ''), participant.email))
          )
        )
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
  options: { includeTranscriptWords?: boolean } = {},
): Promise<SessionContentSnapshot | null> {
  if (!sessionId) return null;
  const rows = await liveQueryClient.execute<SessionContentSqlRow>(
    SESSION_CONTENT_SQL,
    [
      options.includeTranscriptWords === false ? 0 : 1,
      options.includeTranscriptWords === false ? 0 : 1,
      sessionId,
    ],
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
      speakerHintsJson: transcript.speaker_hints_json,
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
      email: participant.email,
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
    ownerEmail: row.owner_email,
    title: row.title,
    createdAt: row.created_at,
    event: parseJson(row.event_json),
    sourceApps: parseSessionSourceApps(row.source_apps_json),
    eventId: row.event_id || null,
    rawNoteId: row.raw_note_id || null,
    rawTemplateId: row.raw_template_id,
    rawUpdatedAt: row.raw_updated_at,
    rawHeadingsUpdatedAt: resolveHeadingsUpdatedAt(
      row.raw_metadata_json,
      row.raw_body,
      row.raw_body_format,
    ),
    rawAppliedTemplate: parseAppliedTemplateSnapshot(row.raw_metadata_json),
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

// Snapshots store section titles only; the merge reads descriptions from the
// live template. Object sections are the pre-release shape and still parse.
const snapshotSectionTitleSchema = z
  .union([
    z.string(),
    z.object({ title: z.string() }).transform((section) => section.title),
  ])
  .transform((title) => title.trim())
  .refine((title) => title.length > 0);

const appliedTemplateSnapshotSchema = z.object({
  appliedTemplate: z.object({
    templateId: z.string(),
    sections: z
      .array(snapshotSectionTitleSchema.nullable().catch(null))
      .transform((titles) =>
        titles.filter((title): title is string => title !== null),
      ),
  }),
});

export function parseAppliedTemplateSnapshot(
  value: string,
): AppliedTemplateSnapshot | null {
  const result = appliedTemplateSnapshotSchema.safeParse(parseJson(value));
  return result.success ? result.data.appliedTemplate : null;
}

const headingsMetadataSchema = z.object({
  headings: z.object({ key: z.string(), updatedAt: z.string() }).nullish(),
});

// Trusts the stored headings timestamp only when its key matches the current
// body: writers outside desktop persistence (e.g. mobile) can change headings
// without restamping. A mismatch or unreadable body falls back to "", letting
// callers use the document write time instead.
export function resolveHeadingsUpdatedAt(
  metadataJson: string,
  body: string,
  bodyFormat: string,
): string {
  const result = headingsMetadataSchema.safeParse(parseJson(metadataJson));
  const stored = result.success ? result.data.headings : null;
  if (!stored) {
    return "";
  }
  return extractBodyHeadingsKey(body, bodyFormat) === stored.key
    ? stored.updatedAt
    : "";
}
