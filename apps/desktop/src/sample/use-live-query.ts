import { useEffect, useState } from "react";

import { subscribe } from "@hypr/plugin-reactive-db";

export interface LiveQueryResult<T> {
  data: T[] | undefined;
  error: string | undefined;
  isLoading: boolean;
}

export function useLiveQuery<T = Record<string, unknown>>(
  sql: string,
  params: unknown[] = [],
  options?: { enabled?: boolean },
): LiveQueryResult<T> {
  const [data, setData] = useState<T[] | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [isLoading, setIsLoading] = useState(true);

  const paramsKey = JSON.stringify(params);

  useEffect(() => {
    if (options?.enabled === false) {
      setIsLoading(false);
      return;
    }

    setIsLoading(true);
    setError(undefined);

    let cancelled = false;
    let cleanup: (() => void) | undefined;

    subscribe<T>(sql, params, {
      onData: (rows) => {
        if (cancelled) return;
        setData(rows);
        setError(undefined);
        setIsLoading(false);
      },
      onError: (msg) => {
        if (cancelled) return;
        setError(msg);
        setIsLoading(false);
      },
    }).then((unsub) => {
      if (cancelled) {
        unsub();
      } else {
        cleanup = unsub;
      }
    });

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [sql, paramsKey, options?.enabled]);

  return { data, error, isLoading };
}
