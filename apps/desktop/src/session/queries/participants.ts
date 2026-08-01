import type { SessionParticipantRecord } from "./types";

import { executeTransaction, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { id } from "~/shared/utils";

type SessionParticipantSqlRow = {
  id: string;
  session_id: string;
  human_id: string;
  source: string;
  name: string;
  email: string;
  job_title: string;
  linkedin_username: string;
  organization_id: string;
  organization_name: string;
};

const EMPTY_SESSION_PARTICIPANTS: SessionParticipantRecord[] = [];

export function useSessionParticipants(
  sessionId: string,
): SessionParticipantRecord[] {
  const { data = EMPTY_SESSION_PARTICIPANTS } = useLiveQuery<
    SessionParticipantSqlRow,
    SessionParticipantRecord[]
  >({
    sql: `
      SELECT
        participant.id,
        participant.session_id,
        participant.human_id,
        participant.source,
        COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
        COALESCE(NULLIF(human.email, ''), participant.email) AS email,
        COALESCE(human.job_title, '') AS job_title,
        COALESCE(human.linkedin_username, '') AS linkedin_username,
        COALESCE(human.organization_id, '') AS organization_id,
        COALESCE(organization.name, '') AS organization_name
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
      LEFT JOIN organizations AS organization
        ON organization.id = human.organization_id
        AND organization.deleted_at IS NULL
      WHERE participant.session_id = ?
        AND participant.deleted_at IS NULL
      ORDER BY name, email, participant.id
    `,
    params: [sessionId],
    enabled: Boolean(sessionId),
    mapRows: (rows) => rows.map(mapSessionParticipantRow),
  });
  return sessionId ? data : EMPTY_SESSION_PARTICIPANTS;
}

export function useSessionParticipant(
  mappingId: string,
): SessionParticipantRecord | null {
  const { data = null } = useLiveQuery<
    SessionParticipantSqlRow,
    SessionParticipantRecord | null
  >({
    sql: `
      SELECT
        participant.id,
        participant.session_id,
        participant.human_id,
        participant.source,
        COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
        COALESCE(NULLIF(human.email, ''), participant.email) AS email,
        COALESCE(human.job_title, '') AS job_title,
        COALESCE(human.linkedin_username, '') AS linkedin_username,
        COALESCE(human.organization_id, '') AS organization_id,
        COALESCE(organization.name, '') AS organization_name
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
      LEFT JOIN organizations AS organization
        ON organization.id = human.organization_id
        AND organization.deleted_at IS NULL
      WHERE participant.id = ? AND participant.deleted_at IS NULL
      LIMIT 1
    `,
    params: [mappingId],
    enabled: Boolean(mappingId),
    mapRows: (rows) => (rows[0] ? mapSessionParticipantRow(rows[0]) : null),
  });
  return mappingId ? data : null;
}

export function addSessionParticipant(
  sessionId: string,
  humanId: string,
  source = "manual",
): Promise<void> {
  return enqueueDatabaseWrite("session-participants", async () => {
    const participantId = id();
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_participants
          SET source = ?, updated_at = ?
          WHERE id = (
            SELECT id
            FROM session_participants
            WHERE session_id = ?
              AND human_id = ?
              AND source = 'excluded'
              AND deleted_at IS NULL
              AND ? <> 'auto'
            ORDER BY created_at, id
            LIMIT 1
          )
        `,
        params: [source, now, sessionId, humanId, source],
      },
      {
        sql: `
          INSERT INTO session_participants (
            id, workspace_id, owner_user_id, session_id, human_id,
            display_name, email, role, source, metadata_json, created_at,
            updated_at, deleted_at
          )
          SELECT ?, session.workspace_id, session.owner_user_id, session.id, human.id,
            human.name, human.email, '', ?, '{}', ?, ?, NULL
          FROM sessions AS session
          JOIN humans AS human ON human.id = ? AND human.deleted_at IS NULL
          WHERE session.id = ?
            AND session.deleted_at IS NULL
            AND NOT EXISTS (
              SELECT 1
              FROM session_participants AS existing
              WHERE existing.session_id = session.id
                AND existing.human_id = human.id
                AND existing.deleted_at IS NULL
            )
        `,
        params: [participantId, source, now, now, humanId, sessionId],
      },
    ]);
  });
}

export function removeSessionParticipant(mappingId: string): Promise<void> {
  return enqueueDatabaseWrite("session-participants", async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_participants
          SET
            source = CASE WHEN source = 'auto' THEN 'excluded' ELSE source END,
            deleted_at = CASE WHEN source = 'auto' THEN NULL ELSE ? END,
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL
        `,
        params: [now, now, mappingId],
      },
    ]);
  });
}

function mapSessionParticipantRow(
  row: SessionParticipantSqlRow,
): SessionParticipantRecord {
  return {
    id: row.id,
    sessionId: row.session_id,
    humanId: row.human_id,
    source: row.source,
    name: row.name,
    email: row.email,
    jobTitle: row.job_title,
    linkedinUsername: row.linkedin_username,
    organizationId: row.organization_id,
    organizationName: row.organization_name,
  };
}
