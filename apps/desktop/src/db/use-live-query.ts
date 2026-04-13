import { useEffect, useMemo, useRef, useState } from "react";

import { subscribe } from "@hypr/plugin-db";

type UseLiveQueryOptions<TRow, TData> = {
  sql: string;
  params?: unknown[];
  mapRows?: (rows: TRow[]) => TData;
  enabled?: boolean;
};

export function useLiveQuery<TRow, TData>({
  sql,
  params = [],
  mapRows,
  enabled = true,
}: UseLiveQueryOptions<TRow, TData>) {
  const paramsKey = useMemo(() => JSON.stringify(params), [params]);
  const mapRowsRef = useRef(mapRows);
  const [data, setData] = useState<TData>();
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  mapRowsRef.current = mapRows;

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;

    if (!enabled) {
      setIsLoading(false);
      setError(null);
      return;
    }

    setData(undefined);
    setIsLoading(true);
    setError(null);

    subscribe<TRow>(sql, params, {
      onData: (rows) => {
        if (!active) {
          return;
        }

        const nextData = mapRowsRef.current
          ? mapRowsRef.current(rows)
          : (rows as TData);
        setData(nextData);
        setIsLoading(false);
        setError(null);
      },
      onError: (message) => {
        if (!active) {
          return;
        }

        setIsLoading(false);
        setError(new Error(message));
      },
    })
      .then((fn) => {
        if (!active) {
          fn();
          return;
        }

        unsubscribe = fn;
      })
      .catch((error) => {
        if (!active) {
          return;
        }

        setIsLoading(false);
        setError(error instanceof Error ? error : new Error(String(error)));
      });

    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [enabled, paramsKey, sql]);

  return { data, isLoading, error };
}
