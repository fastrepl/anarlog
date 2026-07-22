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
const MAX_SHARED_NOTE_ATTACHMENTS = 64;
const MAX_SHARED_ATTACHMENT_BYTES = 512 * 1024 * 1024;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const cacheMutationVersions = new Map<string, number>();

export type SharedNoteCapability = "viewer" | "commenter" | "editor";

export type SharedNoteAttachment = {
  id: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
};

export type SharedNoteSnapshot = {
  shareId: string;
  workspaceId: string;
  sessionId: string;
  schemaVersion: 1;
  contentRevision: number;
  title: string;
  body: JSONContent;
  attachments: SharedNoteAttachment[];
  capability: SharedNoteCapability;
  manageAccess: boolean;
  accessVersion: number;
  webEditable: boolean;
  webEditBase: {
    contentRevision: number;
    title: string;
    body: JSONContent;
  } | null;
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
  attachments_json: unknown;
  capability: string;
  manage_access: number | boolean;
  access_version: number;
  web_editable: number | boolean;
  web_edit_base_content_revision: number | null;
  web_edit_base_title: string | null;
  web_edit_base_body_json: unknown | null;
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
  expectedMutationVersion?: number,
): Promise<boolean> {
  requireIdentity(viewerUserId, "viewer user");

  const statements = [
    {
      sql: `
        UPDATE shared_session_attachment_cache
        SET availability = 'delete_pending',
            claim_token = '',
            attempt_count = 0,
            next_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            last_error = '',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE viewer_user_id = ?
      `,
      params: [viewerUserId],
    },
    {
      sql: "DELETE FROM shared_session_cache WHERE viewer_user_id = ?",
      params: [viewerUserId],
    },
    ...snapshots.flatMap((snapshot) => [
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
          attachments_json,
          capability,
          manage_access,
          access_version,
          web_editable,
          web_edit_base_content_revision,
          web_edit_base_title,
          web_edit_base_body_json,
          published_at,
          cached_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
      `,
        params: sharedNoteParams(viewerUserId, snapshot),
      },
      ...attachmentReconciliationStatements(viewerUserId, snapshot),
    ]),
    sessionShareSyncStatePruneStatement(viewerUserId, snapshots),
  ];

  return enqueueDatabaseWrite(cacheWriteKey(viewerUserId), async () => {
    if (
      expectedMutationVersion !== undefined &&
      expectedMutationVersion !== currentCacheMutationVersion(viewerUserId)
    ) {
      return false;
    }
    await executeTransaction(statements);
    return true;
  });
}

export function captureDurableSharedNoteCacheMutationVersion(
  viewerUserId: string,
) {
  requireIdentity(viewerUserId, "viewer user");
  return currentCacheMutationVersion(viewerUserId);
}

export function enqueueDurableSharedNoteCacheMutation<T>(
  viewerUserId: string,
  write: () => Promise<T>,
): Promise<T> {
  requireIdentity(viewerUserId, "viewer user");
  cacheMutationVersions.set(
    viewerUserId,
    currentCacheMutationVersion(viewerUserId) + 1,
  );
  return enqueueDatabaseWrite(cacheWriteKey(viewerUserId), write);
}

function sessionShareSyncStatePruneStatement(
  viewerUserId: string,
  snapshots: SharedNoteSnapshot[],
) {
  if (snapshots.length === 0) {
    return {
      sql: "DELETE FROM session_share_sync_state WHERE viewer_user_id = ?",
      params: [viewerUserId],
    };
  }
  return {
    sql: `
      DELETE FROM session_share_sync_state
      WHERE viewer_user_id = ?
        AND share_id NOT IN (${snapshots.map(() => "?").join(", ")})
    `,
    params: [viewerUserId, ...snapshots.map((snapshot) => snapshot.shareId)],
  };
}

export async function upsertDurableSharedNoteCache(
  viewerUserId: string,
  snapshot: SharedNoteSnapshot,
): Promise<void> {
  requireIdentity(viewerUserId, "viewer user");

  await enqueueDurableSharedNoteCacheMutation(viewerUserId, () =>
    executeTransaction([
      {
        sql: `
          UPDATE shared_session_attachment_cache
          SET availability = 'delete_pending',
              claim_token = '',
              attempt_count = 0,
              next_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
              last_error = '',
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE viewer_user_id = ? AND share_id = ?
        `,
        params: [viewerUserId, snapshot.shareId],
      },
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
            attachments_json,
            capability,
            manage_access,
            access_version,
            web_editable,
            web_edit_base_content_revision,
            web_edit_base_title,
            web_edit_base_body_json,
            published_at,
            cached_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
          ON CONFLICT(viewer_user_id, share_id) DO UPDATE SET
            viewer_user_id = excluded.viewer_user_id,
            workspace_id = excluded.workspace_id,
            session_id = excluded.session_id,
            schema_version = excluded.schema_version,
            content_revision = excluded.content_revision,
            title = excluded.title,
            body_json = excluded.body_json,
            attachments_json = excluded.attachments_json,
            capability = excluded.capability,
            manage_access = excluded.manage_access,
            access_version = excluded.access_version,
            web_editable = excluded.web_editable,
            web_edit_base_content_revision = excluded.web_edit_base_content_revision,
            web_edit_base_title = excluded.web_edit_base_title,
            web_edit_base_body_json = excluded.web_edit_base_body_json,
            published_at = excluded.published_at,
            cached_at = excluded.cached_at
        `,
        params: sharedNoteParams(viewerUserId, snapshot),
      },
      ...attachmentReconciliationStatements(viewerUserId, snapshot),
    ]),
  );
}

export async function removeDurableSharedNoteCache(
  viewerUserId: string,
  shareId: string,
): Promise<void> {
  requireIdentity(viewerUserId, "viewer user");
  requireUuid(shareId, "share");

  await enqueueDurableSharedNoteCacheMutation(viewerUserId, () =>
    executeTransaction([
      {
        sql: `
          UPDATE shared_session_attachment_cache
          SET availability = 'delete_pending',
              claim_token = '',
              attempt_count = 0,
              next_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
              last_error = '',
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE viewer_user_id = ? AND share_id = ?
        `,
        params: [viewerUserId, shareId],
      },
      {
        sql: `
          DELETE FROM shared_session_cache
          WHERE viewer_user_id = ? AND share_id = ?
        `,
        params: [viewerUserId, shareId],
      },
      {
        sql: `
          DELETE FROM session_share_sync_state
          WHERE viewer_user_id = ? AND share_id = ?
        `,
        params: [viewerUserId, shareId],
      },
    ]),
  );
}

function currentCacheMutationVersion(viewerUserId: string) {
  return cacheMutationVersions.get(viewerUserId) ?? 0;
}

function cacheWriteKey(viewerUserId: string) {
  return `shared-note-cache:${viewerUserId}`;
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
  await flushDatabaseWrites([cacheWriteKey(normalizedViewerUserId)]);
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
      const attachments =
        typeof row.attachments_json === "string"
          ? JSON.parse(row.attachments_json)
          : row.attachments_json;
      return [
        parseSnapshot({
          share_id: row.share_id,
          workspace_id: row.workspace_id,
          session_id: row.session_id,
          schema_version: row.schema_version,
          content_revision: row.content_revision,
          title: row.title,
          body_json: body,
          attachments_json: attachments,
          capability: row.capability,
          manage_access: row.manage_access === true || row.manage_access === 1,
          access_version: row.access_version,
          web_editable: row.web_editable === true || row.web_editable === 1,
          web_edit_base_content_revision: row.web_edit_base_content_revision,
          web_edit_base_title: row.web_edit_base_title,
          web_edit_base_body_json:
            typeof row.web_edit_base_body_json === "string"
              ? JSON.parse(row.web_edit_base_body_json)
              : row.web_edit_base_body_json,
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
  const attachments = requireAttachments(value.attachments_json);
  const capability = requireCapability(value.capability);
  if (typeof value.manage_access !== "boolean") {
    throw new Error("invalid shared-note management capability");
  }
  const accessVersion = requirePositiveInteger(
    value.access_version,
    "access version",
  );
  if (typeof value.web_editable !== "boolean") {
    throw new Error("invalid shared-note web editability");
  }
  const webEditBase = requireWebEditBase(value, {
    contentRevision,
    manageAccess: value.manage_access,
  });
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
    attachments,
    capability,
    manageAccess: value.manage_access,
    accessVersion,
    webEditable: value.web_editable,
    webEditBase,
    publishedAt,
  };
}

function sharedNoteParams(
  viewerUserId: string,
  snapshot: SharedNoteSnapshot,
): unknown[] {
  return [
    snapshot.shareId,
    viewerUserId,
    snapshot.workspaceId,
    snapshot.sessionId,
    snapshot.schemaVersion,
    snapshot.contentRevision,
    snapshot.title,
    JSON.stringify(snapshot.body),
    JSON.stringify(snapshot.attachments),
    snapshot.capability,
    snapshot.manageAccess ? 1 : 0,
    snapshot.accessVersion,
    snapshot.webEditable ? 1 : 0,
    snapshot.webEditBase?.contentRevision ?? null,
    snapshot.webEditBase?.title ?? null,
    snapshot.webEditBase ? JSON.stringify(snapshot.webEditBase.body) : null,
    snapshot.publishedAt,
  ];
}

function requireWebEditBase(
  value: Record<string, unknown>,
  context: { contentRevision: number; manageAccess: boolean },
): SharedNoteSnapshot["webEditBase"] {
  const revision = value.web_edit_base_content_revision;
  const title = value.web_edit_base_title;
  const body = value.web_edit_base_body_json;
  if (revision === null && title === null && body === null) return null;
  if (revision === null || title === null || body === null) {
    throw new Error("invalid shared-note web edit base");
  }
  if (
    !context.manageAccess ||
    !Number.isSafeInteger(revision) ||
    (revision as number) < 1 ||
    (revision as number) >= context.contentRevision
  ) {
    throw new Error("invalid shared-note web edit base");
  }
  return {
    contentRevision: revision as number,
    title: requireTitle(title),
    body: requireDocument(body),
  };
}

function attachmentReconciliationStatements(
  viewerUserId: string,
  snapshot: SharedNoteSnapshot,
) {
  return snapshot.attachments.map((attachment) => ({
    sql: `
      INSERT INTO shared_session_attachment_cache (
        viewer_user_id,
        share_id,
        attachment_id,
        filename,
        content_type,
        size_bytes,
        sha256,
        availability,
        access_version,
        updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
      ON CONFLICT(viewer_user_id, share_id, attachment_id) DO UPDATE SET
        filename = excluded.filename,
        content_type = excluded.content_type,
        size_bytes = excluded.size_bytes,
        sha256 = excluded.sha256,
        cache_id = CASE
          WHEN shared_session_attachment_cache.sha256 = excluded.sha256
            AND shared_session_attachment_cache.size_bytes = excluded.size_bytes
            AND shared_session_attachment_cache.cache_id <> ''
            AND shared_session_attachment_cache.availability IN (
              'present',
              'delete_pending'
            )
          THEN shared_session_attachment_cache.cache_id
          ELSE ''
        END,
        availability = CASE
          WHEN shared_session_attachment_cache.sha256 = excluded.sha256
            AND shared_session_attachment_cache.size_bytes = excluded.size_bytes
            AND shared_session_attachment_cache.cache_id <> ''
            AND shared_session_attachment_cache.availability IN (
              'present',
              'delete_pending'
            )
          THEN 'present'
          ELSE 'pending'
        END,
        access_version = excluded.access_version,
        claim_token = '',
        attempt_count = 0,
        next_attempt_at = excluded.updated_at,
        last_attempt_at = NULL,
        last_error = '',
        updated_at = excluded.updated_at
    `,
    params: [
      viewerUserId,
      snapshot.shareId,
      attachment.id,
      attachment.filename,
      attachment.contentType,
      attachment.sizeBytes,
      attachment.sha256,
      snapshot.accessVersion,
    ],
  }));
}

function requireAttachments(value: unknown): SharedNoteAttachment[] {
  if (!Array.isArray(value) || value.length > MAX_SHARED_NOTE_ATTACHMENTS) {
    throw new Error("invalid shared-note attachments");
  }

  const ids = new Set<string>();
  return value.map((candidate) => {
    if (!isRecord(candidate)) {
      throw new Error("invalid shared-note attachment");
    }
    const id = requireUuid(candidate.id, "attachment");
    if (ids.has(id)) {
      throw new Error("duplicate shared-note attachment");
    }
    ids.add(id);
    if (
      typeof candidate.filename !== "string" ||
      candidate.filename.length === 0 ||
      candidate.filename.length > 1024 ||
      candidate.filename.trim() !== candidate.filename ||
      typeof candidate.contentType !== "string" ||
      candidate.contentType.length === 0 ||
      candidate.contentType.length > 512 ||
      candidate.contentType.trim() !== candidate.contentType ||
      !Number.isSafeInteger(candidate.sizeBytes) ||
      (candidate.sizeBytes as number) < 0 ||
      (candidate.sizeBytes as number) > MAX_SHARED_ATTACHMENT_BYTES ||
      typeof candidate.sha256 !== "string" ||
      !SHA256_PATTERN.test(candidate.sha256)
    ) {
      throw new Error("invalid shared-note attachment");
    }
    return {
      id,
      filename: candidate.filename,
      contentType: candidate.contentType,
      sizeBytes: candidate.sizeBytes as number,
      sha256: candidate.sha256,
    };
  });
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
