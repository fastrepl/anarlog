import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite, flushDatabaseWrites } from "~/db/write-queue";

export type SharedAttachmentCacheJob = {
  viewerUserId: string;
  shareId: string;
  attachmentId: string;
  filename: string;
  contentType: string;
  sizeBytes: number;
  sha256: string;
  cacheId: string;
  claimToken: string;
  availability: "downloading" | "deleting";
  accessVersion: number;
  attemptCount: number;
};

type CacheSqlRow = {
  viewer_user_id: string;
  share_id: string;
  attachment_id: string;
  filename: string;
  content_type: string;
  size_bytes: number;
  sha256: string;
  cache_id: string;
  availability: string;
  access_version: number;
  attempt_count: number;
};

const WRITE_KEY = "shared-attachment-cache-runner";

export const sharedAttachmentCacheStore = {
  async recoverInterrupted(viewerUserId: string) {
    await enqueueDatabaseWrite(WRITE_KEY, () =>
      executeTransaction([
        {
          sql: `
            UPDATE shared_session_attachment_cache
            SET availability = CASE
                  WHEN availability = 'downloading' THEN 'pending'
                  ELSE 'delete_pending'
                END,
                claim_token = '',
                next_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE viewer_user_id = ?
              AND availability IN ('downloading', 'deleting')
          `,
          params: [viewerUserId],
        },
      ]),
    );
  },

  async claimNext(
    viewerUserId: string,
  ): Promise<SharedAttachmentCacheJob | null> {
    await flushDatabaseWrites();
    const rows = await liveQueryClient.execute<CacheSqlRow>(
      `
        SELECT
          viewer_user_id,
          share_id,
          attachment_id,
          filename,
          content_type,
          size_bytes,
          sha256,
          cache_id,
          availability,
          access_version,
          attempt_count
        FROM shared_session_attachment_cache
        WHERE viewer_user_id = ?
          AND availability IN ('pending', 'delete_pending', 'failed')
          AND next_attempt_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ORDER BY
          CASE WHEN availability = 'delete_pending' THEN 0 ELSE 1 END,
          next_attempt_at,
          updated_at,
          attachment_id
        LIMIT 1
      `,
      [viewerUserId],
    );
    const row = rows[0];
    if (!row) return null;
    const claimedAvailability =
      row.availability === "delete_pending" ? "deleting" : "downloading";
    const claimToken = crypto.randomUUID();
    const results = await enqueueDatabaseWrite(WRITE_KEY, () =>
      executeTransaction([
        {
          sql: `
            UPDATE shared_session_attachment_cache
            SET availability = ?,
                claim_token = ?,
                attempt_count = attempt_count + 1,
                last_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE viewer_user_id = ?
              AND share_id = ?
              AND attachment_id = ?
              AND availability = ?
          `,
          params: [
            claimedAvailability,
            claimToken,
            row.viewer_user_id,
            row.share_id,
            row.attachment_id,
            row.availability,
          ],
        },
      ]),
    );
    if ((results[0] ?? 0) !== 1) return null;
    return {
      viewerUserId: row.viewer_user_id,
      shareId: row.share_id,
      attachmentId: row.attachment_id,
      filename: row.filename,
      contentType: row.content_type,
      sizeBytes: row.size_bytes,
      sha256: row.sha256,
      cacheId: row.cache_id,
      claimToken,
      availability: claimedAvailability,
      accessVersion: row.access_version,
      attemptCount: row.attempt_count + 1,
    };
  },

  async completeDownload(job: SharedAttachmentCacheJob, cacheId: string) {
    const results = await enqueueDatabaseWrite(WRITE_KEY, () =>
      executeTransaction([
        {
          sql: `
            UPDATE shared_session_attachment_cache
            SET cache_id = ?,
                availability = 'present',
                cache_generation = cache_generation + 1,
                attempt_count = 0,
                next_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                last_error = '',
                claim_token = '',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE viewer_user_id = ?
              AND share_id = ?
              AND attachment_id = ?
              AND availability = 'downloading'
              AND sha256 = ?
              AND size_bytes = ?
              AND access_version = ?
              AND claim_token = ?
          `,
          params: [
            cacheId,
            job.viewerUserId,
            job.shareId,
            job.attachmentId,
            job.sha256,
            job.sizeBytes,
            job.accessVersion,
            job.claimToken,
          ],
        },
      ]),
    );
    return (results[0] ?? 0) === 1;
  },

  async completeDelete(job: SharedAttachmentCacheJob) {
    const results = await enqueueDatabaseWrite(WRITE_KEY, () =>
      executeTransaction([
        {
          sql: `
            DELETE FROM shared_session_attachment_cache
            WHERE viewer_user_id = ?
              AND share_id = ?
              AND attachment_id = ?
              AND availability = 'deleting'
              AND claim_token = ?
          `,
          params: [
            job.viewerUserId,
            job.shareId,
            job.attachmentId,
            job.claimToken,
          ],
        },
        {
          sql: `
            UPDATE shared_session_attachment_cache
            SET cache_id = '',
                availability = 'pending',
                cache_generation = cache_generation + 1,
                claim_token = '',
                attempt_count = 0,
                next_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                last_error = '',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE viewer_user_id = ?
              AND share_id = ?
              AND attachment_id = ?
              AND availability = 'present'
              AND cache_id = ?
          `,
          params: [
            job.viewerUserId,
            job.shareId,
            job.attachmentId,
            job.cacheId,
          ],
        },
      ]),
    );
    return (results[0] ?? 0) === 1 || (results[1] ?? 0) === 1;
  },

  async retry(job: SharedAttachmentCacheJob, message: string, retryAt: Date) {
    const availability =
      job.availability === "deleting" ? "delete_pending" : "failed";
    await enqueueDatabaseWrite(WRITE_KEY, () =>
      executeTransaction([
        {
          sql: `
            UPDATE shared_session_attachment_cache
            SET availability = ?,
                claim_token = '',
                next_attempt_at = ?,
                last_error = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE viewer_user_id = ?
              AND share_id = ?
              AND attachment_id = ?
              AND availability = ?
              AND claim_token = ?
          `,
          params: [
            availability,
            retryAt.toISOString(),
            message.slice(0, 2048),
            job.viewerUserId,
            job.shareId,
            job.attachmentId,
            job.availability,
            job.claimToken,
          ],
        },
      ]),
    );
  },

  async markDeletePending(job: SharedAttachmentCacheJob) {
    await enqueueDatabaseWrite(WRITE_KEY, () =>
      executeTransaction([
        {
          sql: `
            UPDATE shared_session_attachment_cache
            SET availability = 'delete_pending',
                claim_token = '',
                next_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                last_error = '',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE viewer_user_id = ?
              AND share_id = ?
              AND attachment_id = ?
              AND availability = 'downloading'
              AND claim_token = ?
          `,
          params: [
            job.viewerUserId,
            job.shareId,
            job.attachmentId,
            job.claimToken,
          ],
        },
      ]),
    );
  },

  async listPresent(viewerUserId: string) {
    await flushDatabaseWrites();
    return liveQueryClient.execute<
      Pick<CacheSqlRow, "share_id" | "attachment_id">
    >(
      `
        SELECT share_id, attachment_id
        FROM shared_session_attachment_cache
        WHERE viewer_user_id = ? AND availability = 'present'
        ORDER BY updated_at
        LIMIT 100
      `,
      [viewerUserId],
    );
  },

  async markMissing(
    viewerUserId: string,
    shareId: string,
    attachmentId: string,
    sha256: string,
    accessVersion: number,
  ) {
    await enqueueDatabaseWrite(WRITE_KEY, () =>
      executeTransaction([
        {
          sql: `
            UPDATE shared_session_attachment_cache
            SET cache_id = '',
                availability = 'pending',
                attempt_count = 0,
                next_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                last_error = '',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE viewer_user_id = ?
              AND share_id = ?
              AND attachment_id = ?
              AND availability = 'present'
              AND sha256 = ?
              AND access_version = ?
          `,
          params: [viewerUserId, shareId, attachmentId, sha256, accessVersion],
        },
      ]),
    );
  },
};

export async function purgeViewerSharedNoteCache(
  viewerUserId: string,
  removeScope: (scopeId: string) => Promise<unknown>,
) {
  await removeScope(viewerUserId);
  await enqueueDatabaseWrite(WRITE_KEY, () =>
    executeTransaction([
      {
        sql: "DELETE FROM shared_session_attachment_cache WHERE viewer_user_id = ?",
        params: [viewerUserId],
      },
      {
        sql: "DELETE FROM shared_session_cache WHERE viewer_user_id = ?",
        params: [viewerUserId],
      },
    ]),
  );
}

export async function purgeForeignViewerSharedNoteCaches(
  activeViewerUserId: string,
  removeScope: (scopeId: string) => Promise<unknown>,
) {
  await flushDatabaseWrites();
  const viewers = await liveQueryClient.execute<{ viewer_user_id: string }>(
    `
      SELECT DISTINCT viewer_user_id
      FROM (
        SELECT viewer_user_id FROM shared_session_attachment_cache
        UNION
        SELECT viewer_user_id FROM shared_session_cache
      )
      WHERE viewer_user_id <> ?
    `,
    [activeViewerUserId],
  );
  for (const viewer of viewers) {
    await purgeViewerSharedNoteCache(viewer.viewer_user_id, removeScope);
  }
}
