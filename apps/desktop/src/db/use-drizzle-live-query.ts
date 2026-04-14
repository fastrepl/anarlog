import { useLiveQuery } from "./use-live-query";

type DrizzleQuery = { toSQL(): { sql: string; params: unknown[] } };

export function useDrizzleLiveQuery<TRow, TData = TRow[]>(
  query: DrizzleQuery,
  options?: { mapRows?: (rows: TRow[]) => TData; enabled?: boolean },
) {
  const { sql, params } = query.toSQL();

  return useLiveQuery<TRow, TData>({
    sql,
    params,
    mapRows: options?.mapRows,
    enabled: options?.enabled,
  });
}
