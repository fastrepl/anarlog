import { execute } from "@anlg/plugin-db";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

export async function flushCloudsyncSessionEvictions(
  shouldStop: () => boolean,
): Promise<boolean> {
  const batchSize = 128;

  while (true) {
    if (shouldStop()) {
      return false;
    }

    let rows: { sessionId: string; workspaceId: string }[];
    try {
      rows = await execute(
        `
          SELECT
            eviction.session_id AS sessionId,
            eviction.workspace_id AS workspaceId
          FROM cloudsync_session_evictions AS eviction
          WHERE NOT EXISTS (
            SELECT 1
            FROM workspace_memberships AS membership
            WHERE membership.workspace_id = eviction.workspace_id
              AND membership.deleted_at IS NULL
          )
          AND NOT EXISTS (
            SELECT 1
            FROM sessions
            WHERE sessions.id = eviction.session_id
          )
          ORDER BY eviction.queued_at, eviction.session_id
          LIMIT ?
        `,
        [batchSize],
      );
    } catch (error) {
      console.warn("[cloudsync] session eviction queue unavailable", error);
      return true;
    }

    if (shouldStop()) {
      return false;
    }
    if (rows.length === 0) return false;

    let failed = false;
    for (const row of rows) {
      if (shouldStop()) {
        return false;
      }

      let deletionError = "";
      try {
        const result = await fsSyncCommands.deleteSessionFolder(row.sessionId);
        if (result.status === "error") {
          deletionError = String(result.error);
        }
      } catch (error) {
        deletionError = error instanceof Error ? error.message : String(error);
      }

      if (shouldStop()) {
        return false;
      }

      try {
        if (deletionError) {
          failed = true;
          await execute(
            `
              UPDATE cloudsync_session_evictions
              SET attempt_count = attempt_count + 1,
                  last_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                  last_error = ?
              WHERE session_id = ? AND workspace_id = ?
            `,
            [deletionError.slice(0, 512), row.sessionId, row.workspaceId],
          );
          continue;
        }

        await execute(
          `
            DELETE FROM cloudsync_session_evictions
            WHERE session_id = ? AND workspace_id = ?
              AND NOT EXISTS (
                SELECT 1
                FROM workspace_memberships
                WHERE workspace_id = ? AND deleted_at IS NULL
              )
              AND NOT EXISTS (
                SELECT 1 FROM sessions WHERE id = ?
              )
          `,
          [row.sessionId, row.workspaceId, row.workspaceId, row.sessionId],
        );
      } catch (error) {
        failed = true;
        console.warn(
          "[cloudsync] failed to update session eviction queue",
          error,
        );
      }
    }

    if (failed) return true;
    if (rows.length < batchSize) return false;
  }
}
