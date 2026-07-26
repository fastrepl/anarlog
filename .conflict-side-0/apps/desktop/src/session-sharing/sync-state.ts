import { useLiveQuery } from "~/db";

type SessionShareSyncStatusRow = {
  status: string;
};

export type SessionShareSyncStatus = "clean" | "conflict";

export function useSessionShareSyncStatus(
  viewerUserId: string,
  shareId: string,
  sessionId: string,
): SessionShareSyncStatus | null {
  const enabled = Boolean(viewerUserId && shareId && sessionId);
  const { data = null } = useLiveQuery<
    SessionShareSyncStatusRow,
    SessionShareSyncStatus | null
  >({
    sql: `
      SELECT status
      FROM session_share_sync_state
      WHERE viewer_user_id = ?
        AND share_id = ?
        AND session_id = ?
      LIMIT 1
    `,
    params: [viewerUserId, shareId, sessionId],
    enabled,
    mapRows: (rows) => {
      const status = rows[0]?.status;
      if (status === undefined) return null;
      if (status !== "clean" && status !== "conflict") {
        throw new Error("Invalid shared-note sync status");
      }
      return status;
    },
  });

  return enabled ? data : null;
}
