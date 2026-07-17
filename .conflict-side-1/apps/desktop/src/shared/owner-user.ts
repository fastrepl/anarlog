import { useLiveQuery } from "~/db";
import { DEFAULT_USER_ID } from "~/shared/utils";

type OwnerUserSqlRow = {
  user_id: string;
};

const OWNER_USER_SQL = `
  SELECT user_id
  FROM (
    SELECT owner_user_id AS user_id, updated_at, 0 AS source_priority
    FROM sessions
    WHERE owner_user_id <> '' AND deleted_at IS NULL

    UNION ALL

    SELECT id AS user_id, updated_at, 1 AS source_priority
    FROM humans
    WHERE id = owner_user_id AND id <> '' AND deleted_at IS NULL

    UNION ALL

    SELECT owner_user_id AS user_id, updated_at, 2 AS source_priority
    FROM chat_groups
    WHERE owner_user_id <> '' AND deleted_at IS NULL
  )
  ORDER BY source_priority, updated_at DESC, user_id
  LIMIT 1
`;

export function useOwnerUserId(): string | null {
  const { data } = useLiveQuery<OwnerUserSqlRow, string>({
    sql: OWNER_USER_SQL,
    mapRows: (rows) => rows[0]?.user_id.trim() || DEFAULT_USER_ID,
  });
  return data ?? null;
}
