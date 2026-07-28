import { useEffect, useRef } from "react";

import {
  deleteCloudApiSnapshotBestEffort,
  initializeCloudApiBackfill,
  scheduleCloudApiBackfillRetry,
  scheduleCloudApiSnapshotSync,
} from "./client";

import { useAuth } from "~/auth";
import { useLiveQuery } from "~/db";

type SnapshotRevision = {
  deletedAt: string | null;
  id: string;
  revision: string;
};

export function cloudApiSnapshotChanges(
  previous: ReadonlyMap<string, string> | null,
  revisions: SnapshotRevision[],
) {
  const next = new Map<string, string>();
  const deletes: string[] = [];
  const upserts: string[] = [];

  for (const row of revisions) {
    const state = row.deletedAt ? `deleted:${row.deletedAt}` : row.revision;
    next.set(row.id, state);
    if (row.deletedAt) {
      if (previous?.get(row.id) !== state) {
        deletes.push(row.id);
      }
    } else if (!previous || previous.get(row.id) !== row.revision) {
      upserts.push(row.id);
    }
  }

  if (previous) {
    for (const sessionId of previous.keys()) {
      if (!next.has(sessionId)) {
        deletes.push(sessionId);
      }
    }
  }

  return { deletes, next, upserts };
}

export function CloudApiBackfillLifecycle() {
  const { session } = useAuth();
  const userId = session?.user.id;
  const knownRevisions = useRef<Map<string, string> | null>(null);
  const { data: revisions } = useLiveQuery<
    SnapshotRevision,
    SnapshotRevision[]
  >({
    sql: `
      SELECT
        session.id,
        session.deleted_at AS deletedAt,
        max(
          session.updated_at,
          COALESCE((
            SELECT max(document.updated_at)
            FROM session_documents AS document
            WHERE document.session_id = session.id
          ), ''),
          COALESCE((
            SELECT max(transcript.updated_at)
            FROM transcripts AS transcript
            WHERE transcript.session_id = session.id
          ), ''),
          COALESCE((
            SELECT max(participant.updated_at)
            FROM session_participants AS participant
            WHERE participant.session_id = session.id
          ), ''),
          COALESCE((
            SELECT max(human.updated_at)
            FROM session_participants AS participant
            JOIN humans AS human ON human.id = participant.human_id
            WHERE participant.session_id = session.id
          ), ''),
          COALESCE((
            SELECT max(organization.updated_at)
            FROM session_participants AS participant
            JOIN humans AS human ON human.id = participant.human_id
            JOIN organizations AS organization
              ON organization.id = human.organization_id
            WHERE participant.session_id = session.id
          ), ''),
          COALESCE((
            SELECT max(action_item.updated_at)
            FROM action_items AS action_item
            WHERE action_item.session_id = session.id
          ), '')
        ) AS revision
      FROM sessions AS session
      ORDER BY session.id
    `,
    enabled: Boolean(userId) && session?.user.is_anonymous !== true,
    mapRows: (rows) => rows,
  });

  useEffect(() => {
    knownRevisions.current = userId
      ? readStoredSnapshotRevisions(userId)
      : null;
  }, [userId]);

  useEffect(() => {
    if (!userId || session?.user.is_anonymous === true) {
      return;
    }
    void initializeCloudApiBackfill().catch((error: unknown) => {
      scheduleCloudApiBackfillRetry();
      console.error(
        "[cloud-api] backfill failed",
        error instanceof Error ? error.name : typeof error,
      );
    });
  }, [userId, session?.user.is_anonymous]);

  useEffect(() => {
    if (!userId || session?.user.is_anonymous === true || !revisions) {
      return;
    }
    const changes = cloudApiSnapshotChanges(knownRevisions.current, revisions);
    knownRevisions.current = changes.next;
    storeSnapshotRevisions(userId, changes.next);
    for (const sessionId of changes.deletes) {
      deleteCloudApiSnapshotBestEffort(sessionId);
    }
    for (const sessionId of changes.upserts) {
      scheduleCloudApiSnapshotSync(sessionId);
    }
  }, [revisions, session?.user.is_anonymous, userId]);

  return null;
}

function snapshotRevisionsKey(userId: string) {
  return `anarlog.cloud-api-revisions.v1.${userId}`;
}

function readStoredSnapshotRevisions(userId: string) {
  try {
    const entries = JSON.parse(
      localStorage.getItem(snapshotRevisionsKey(userId)) ?? "null",
    ) as unknown;
    if (
      !Array.isArray(entries) ||
      entries.some(
        (entry) =>
          !Array.isArray(entry) ||
          entry.length !== 2 ||
          entry.some((value) => typeof value !== "string"),
      )
    ) {
      return null;
    }
    return new Map<string, string>(entries);
  } catch {
    return null;
  }
}

function storeSnapshotRevisions(
  userId: string,
  revisions: ReadonlyMap<string, string>,
) {
  try {
    localStorage.setItem(
      snapshotRevisionsKey(userId),
      JSON.stringify([...revisions]),
    );
  } catch {
    return;
  }
}
