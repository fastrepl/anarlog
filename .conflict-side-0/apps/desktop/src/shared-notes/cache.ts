import { and, desc, eq, sharedSessionCache } from "@hypr/db";
import type { JSONContent } from "@hypr/editor/note";

import {
  db,
  executeTransaction,
  liveQueryClient,
  useDrizzleLiveQuery,
} from "~/db";
import { enqueueDatabaseWrite, flushDatabaseWrites } from "~/db/write-queue";

const MAX_SHARED_NOTE_BODY_BYTES = 2 * 1024 * 1024;
const MAX_SHARED_NOTE_TITLE_BYTES = 4096;
const MAX_DURABLE_SHARED_NOTES = 5000;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export type SharedNoteCapability = "viewer" | "commenter" | "editor";

export type SharedNoteSnapshot = {
  shareId: string;
  workspaceId: string;
  sessionId: string;
  schemaVersion: 1;
  contentRevision: number;
  title: string;
  body: JSONContent;
  capability: SharedNoteCapability;
  manageAccess: boolean;
  accessVersion: number;
  publishedAt: string;
};

type SharedNoteLiveRow = {
  share_id: string;
  viewer_user_id: string;
  workspace_id: string;
  session_id: string;
  schema_version: number;
  content_revision: number;
  title: string;
  body_json: unknown;
  capability: string;
  manage_access: number | boolean;
  access_version: number;
  published_at: string;
  cached_at: string;
};

type ManagedSharedNoteSqlRow = {
  share_id: string;
  workspace_id: string;
  session_id: string;
};

export function parseDurableSharedNoteSnapshots(
  value: unknown,
): SharedNoteSnapshot[] {
  if (!Array.isArray(value) || value.length > MAX_DURABLE_SHARED_NOTES) {
    throw new Error("invalid durable shared-note snapshot list");
  }

  const shareIds = new Set<string>();
  return value.map((row) => {
    const snapshot = parseSnapshot(row);
    if (shareIds.has(snapshot.shareId)) {
      throw new Error("duplicate durable shared-note snapshot");
    }
    shareIds.add(snapshot.shareId);
    return snapshot;
  });
}

export async function replaceDurableSharedNoteCache(
  viewerUserId: string,
  snapshots: SharedNoteSnapshot[],
): Promise<void> {
  requireIdentity(viewerUserId, "viewer user");

  const statements = [
    {
      sql: "DELETE FROM shared_session_cache WHERE viewer_user_id = ?",
      params: [viewerUserId],
    },
    ...snapshots.map((snapshot) => ({
      sql: `
        INSERT INTO shared_session_cache (
          share_id,
          viewer_user_id,
          workspace_id,
          session_id,
          schema_version,
          content_revision,
          title,
          body_json,
          capability,
          manage_access,
          access_version,
          published_at,
          cached_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
      `,
      params: [
        snapshot.shareId,
        viewerUserId,
        snapshot.workspaceId,
        snapshot.sessionId,
        snapshot.schemaVersion,
        snapshot.contentRevision,
        snapshot.title,
        JSON.stringify(snapshot.body),
        snapshot.capability,
        snapshot.manageAccess ? 1 : 0,
        snapshot.accessVersion,
        snapshot.publishedAt,
      ],
    })),
  ];

  await enqueueDatabaseWrite(`shared-note-cache:${viewerUserId}`, () =>
    executeTransaction(statements),
  );
}

export async function upsertDurableSharedNoteCache(
  viewerUserId: string,
  snapshot: SharedNoteSnapshot,
): Promise<void> {
  requireIdentity(viewerUserId, "viewer user");

  await enqueueDatabaseWrite(`shared-note-cache:${viewerUserId}`, () =>
    executeTransaction([
      {
        sql: `
          INSERT INTO shared_session_cache (
            share_id,
            viewer_user_id,
            workspace_id,
            session_id,
            schema_version,
            content_revision,
            title,
            body_json,
            capability,
            manage_access,
            access_version,
            published_at,
            cached_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
          ON CONFLICT(viewer_user_id, share_id) DO UPDATE SET
            viewer_user_id = excluded.viewer_user_id,
            workspace_id = excluded.workspace_id,
            session_id = excluded.session_id,
            schema_version = excluded.schema_version,
            content_revision = excluded.content_revision,
            title = excluded.title,
            body_json = excluded.body_json,
            capability = excluded.capability,
            manage_access = excluded.manage_access,
            access_version = excluded.access_version,
            published_at = excluded.published_at,
            cached_at = excluded.cached_at
        `,
        params: [
          snapshot.shareId,
          viewerUserId,
          snapshot.workspaceId,
          snapshot.sessionId,
          snapshot.schemaVersion,
          snapshot.contentRevision,
          snapshot.title,
          JSON.stringify(snapshot.body),
          snapshot.capability,
          snapshot.manageAccess ? 1 : 0,
          snapshot.accessVersion,
          snapshot.publishedAt,
        ],
      },
    ]),
  );
}

export async function removeDurableSharedNoteCache(
  viewerUserId: string,
  shareId: string,
): Promise<void> {
  requireIdentity(viewerUserId, "viewer user");
  requireUuid(shareId, "share");

  await enqueueDatabaseWrite(`shared-note-cache:${viewerUserId}`, () =>
    executeTransaction([
      {
        sql: `
          DELETE FROM shared_session_cache
          WHERE viewer_user_id = ? AND share_id = ?
        `,
        params: [viewerUserId, shareId],
      },
    ]),
  );
}

export async function loadManagedSharedNoteForSession(
  viewerUserId: string,
  sessionId: string,
): Promise<{
  shareId: string;
  workspaceId: string;
  sessionId: string;
} | null> {
  const normalizedViewerUserId = requireIdentity(viewerUserId, "viewer user");
  const normalizedSessionId = requireIdentity(sessionId, "session");
  await flushDatabaseWrites();
  const rows = await liveQueryClient.execute<ManagedSharedNoteSqlRow>(
    `
      SELECT share_id, workspace_id, session_id
      FROM shared_session_cache
      WHERE viewer_user_id = ?
        AND session_id = ?
        AND manage_access = 1
      LIMIT 2
    `,
    [normalizedViewerUserId, normalizedSessionId],
  );
  if (rows.length > 1) {
    throw new Error("ambiguous managed shared-note cache entry");
  }
  const row = rows[0];
  if (!row) return null;
  return {
    shareId: requireUuid(row.share_id, "share"),
    workspaceId: requireUuid(row.workspace_id, "workspace"),
    sessionId: requireIdentity(row.session_id, "session"),
  };
}

export function useDurableSharedNotes(viewerUserId: string | null | undefined) {
  const query = db
    .select()
    .from(sharedSessionCache)
    .where(eq(sharedSessionCache.viewerUserId, viewerUserId ?? ""))
    .orderBy(
      desc(sharedSessionCache.publishedAt),
      desc(sharedSessionCache.shareId),
    );

  const { data = [] } = useDrizzleLiveQuery<
    SharedNoteLiveRow,
    SharedNoteSnapshot[]
  >(query, {
    mapRows: mapSharedNoteLiveRows,
    enabled: Boolean(viewerUserId),
  });

  return viewerUserId ? data : [];
}

export function useDurableSharedNote(
  viewerUserId: string | null | undefined,
  shareId: string,
) {
  const query = db
    .select()
    .from(sharedSessionCache)
    .where(
      and(
        eq(sharedSessionCache.viewerUserId, viewerUserId ?? ""),
        eq(sharedSessionCache.shareId, shareId),
      ),
    )
    .limit(1);

  return useDrizzleLiveQuery<SharedNoteLiveRow, SharedNoteSnapshot | null>(
    query,
    {
      mapRows: (rows) => mapSharedNoteLiveRows(rows)[0] ?? null,
      enabled: Boolean(viewerUserId && shareId),
    },
  );
}

export function mapSharedNoteLiveRows(
  rows: SharedNoteLiveRow[],
): SharedNoteSnapshot[] {
  return rows.flatMap((row) => {
    try {
      const body =
        typeof row.body_json === "string"
          ? JSON.parse(row.body_json)
          : row.body_json;
      return [
        parseSnapshot({
          share_id: row.share_id,
          workspace_id: row.workspace_id,
          session_id: row.session_id,
          schema_version: row.schema_version,
          content_revision: row.content_revision,
          title: row.title,
          body_json: body,
          capability: row.capability,
          manage_access: row.manage_access === true || row.manage_access === 1,
          access_version: row.access_version,
          published_at: row.published_at,
        }),
      ];
    } catch {
      return [];
    }
  });
}

function parseSnapshot(value: unknown): SharedNoteSnapshot {
  if (!isRecord(value)) {
    throw new Error("invalid durable shared-note snapshot");
  }

  const shareId = requireUuid(value.share_id, "share");
  const workspaceId = requireUuid(value.workspace_id, "workspace");
  const sessionId = requireIdentity(value.session_id, "session");
  const schemaVersion = value.schema_version;
  const contentRevision = requirePositiveInteger(
    value.content_revision,
    "content revision",
  );
  const title = requireTitle(value.title);
  const body = requireDocument(value.body_json);
  const capability = requireCapability(value.capability);
  if (typeof value.manage_access !== "boolean") {
    throw new Error("invalid shared-note management capability");
  }
  const accessVersion = requirePositiveInteger(
    value.access_version,
    "access version",
  );
  const publishedAt = requireTimestamp(value.published_at);

  if (schemaVersion !== 1) {
    throw new Error("unsupported shared-note schema version");
  }

  return {
    shareId,
    workspaceId,
    sessionId,
    schemaVersion,
    contentRevision,
    title,
    body,
    capability,
    manageAccess: value.manage_access,
    accessVersion,
    publishedAt,
  };
}

function requireUuid(value: unknown, label: string): string {
  const identity = requireIdentity(value, label);
  if (!UUID_PATTERN.test(identity)) {
    throw new Error(`invalid shared-note ${label} ID`);
  }
  return identity;
}

function requireIdentity(value: unknown, label: string): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 512 ||
    value.trim() !== value
  ) {
    throw new Error(`invalid shared-note ${label} ID`);
  }
  return value;
}

function requirePositiveInteger(value: unknown, label: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 1) {
    throw new Error(`invalid shared-note ${label}`);
  }
  return value as number;
}

function requireTitle(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    utf8Length(value) > MAX_SHARED_NOTE_TITLE_BYTES
  ) {
    throw new Error("invalid shared-note title");
  }
  return value;
}

function requireDocument(value: unknown): JSONContent {
  if (!isRecord(value) || value.type !== "doc") {
    throw new Error("invalid shared-note document");
  }

  const encoded = JSON.stringify(value);
  if (utf8Length(encoded) > MAX_SHARED_NOTE_BODY_BYTES) {
    throw new Error("shared-note document is too large");
  }
  return value as JSONContent;
}

function requireCapability(value: unknown): SharedNoteCapability {
  if (value !== "viewer" && value !== "commenter" && value !== "editor") {
    throw new Error("invalid shared-note capability");
  }
  return value;
}

function requireTimestamp(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    !Number.isFinite(Date.parse(value))
  ) {
    throw new Error("invalid shared-note publication timestamp");
  }
  return value;
}

function utf8Length(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
