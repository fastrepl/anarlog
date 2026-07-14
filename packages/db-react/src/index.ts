import { useMemo, useRef, useSyncExternalStore } from "react";

import type { LiveQueryClient, Unsubscribe } from "@hypr/db-runtime";

type UseLiveQueryOptions<TRow, TData> = {
  sql: string;
  params?: unknown[];
  mapRows?: (rows: TRow[]) => TData;
  enabled?: boolean;
};

type DrizzleQuery = { toSQL(): { sql: string; params: unknown[] } };

type QuerySnapshot = {
  rows: unknown[] | undefined;
  isLoading: boolean;
  error: Error | null;
};

type SharedQuery = {
  subscribe: (listener: () => void) => () => void;
  getSnapshot: () => QuerySnapshot;
};

const LOADING_SNAPSHOT: QuerySnapshot = {
  rows: undefined,
  isLoading: true,
  error: null,
};

const DISABLED_SNAPSHOT: QuerySnapshot = {
  rows: undefined,
  isLoading: false,
  error: null,
};

const DISABLED_QUERY: SharedQuery = {
  subscribe: () => () => {},
  getSnapshot: () => DISABLED_SNAPSHOT,
};

function createQueryHandle(acquire: () => SharedQuery): SharedQuery {
  let activeQuery: SharedQuery | undefined;
  let activeSubscriptions = 0;

  return {
    subscribe: (listener) => {
      const query = acquire();
      activeQuery = query;
      activeSubscriptions += 1;
      const release = query.subscribe(listener);

      return () => {
        release();
        activeSubscriptions -= 1;
        if (activeSubscriptions === 0) {
          activeQuery = undefined;
        }
      };
    },
    getSnapshot: () => activeQuery?.getSnapshot() ?? LOADING_SNAPSHOT,
  };
}

function createSharedQuery(
  client: LiveQueryClient,
  sql: string,
  params: unknown[],
  onEmpty: () => void,
): SharedQuery {
  let snapshot = LOADING_SNAPSHOT;
  let unsubscribe: Unsubscribe | undefined;
  let generation = 0;
  let started = false;
  const listeners = new Set<() => void>();

  const update = (nextSnapshot: QuerySnapshot) => {
    snapshot = nextSnapshot;
    listeners.forEach((listener) => listener());
  };

  const start = () => {
    if (started) {
      return;
    }

    started = true;
    const currentGeneration = ++generation;
    let subscription: Promise<Unsubscribe>;

    try {
      subscription = client.subscribe<unknown>(sql, params, {
        onData: (rows) => {
          if (!started || generation !== currentGeneration) {
            return;
          }

          update({ rows, isLoading: false, error: null });
        },
        onError: (message) => {
          if (!started || generation !== currentGeneration) {
            return;
          }

          update({
            ...snapshot,
            isLoading: false,
            error: new Error(message),
          });
        },
      });
    } catch (error) {
      update({
        ...snapshot,
        isLoading: false,
        error: error instanceof Error ? error : new Error(String(error)),
      });
      return;
    }

    void subscription
      .then((stop) => {
        if (!started || generation !== currentGeneration) {
          void stop().catch(() => {});
          return;
        }

        unsubscribe = stop;
      })
      .catch((error) => {
        if (!started || generation !== currentGeneration) {
          return;
        }

        update({
          ...snapshot,
          isLoading: false,
          error: error instanceof Error ? error : new Error(String(error)),
        });
      });
  };

  const stop = () => {
    if (!started) {
      return;
    }

    started = false;
    generation += 1;
    const stopSubscription = unsubscribe;
    unsubscribe = undefined;
    void stopSubscription?.().catch(() => {});
  };

  return {
    subscribe: (listener) => {
      listeners.add(listener);
      start();

      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
          stop();
          onEmpty();
        }
      };
    },
    getSnapshot: () => snapshot,
  };
}

export function createUseLiveQuery(client: LiveQueryClient) {
  const sharedQueries = new Map<string, SharedQuery>();

  const getSharedQuery = (key: string, sql: string, params: unknown[]) => {
    const existing = sharedQueries.get(key);
    if (existing) {
      return existing;
    }

    let query: SharedQuery;
    query = createSharedQuery(client, sql, params, () => {
      if (sharedQueries.get(key) === query) {
        sharedQueries.delete(key);
      }
    });
    sharedQueries.set(key, query);
    return query;
  };

  return function useLiveQuery<TRow, TData>({
    sql,
    params = [],
    mapRows,
    enabled = true,
  }: UseLiveQueryOptions<TRow, TData>) {
    const paramsKey = useMemo(() => JSON.stringify(params), [params]);
    const stableParams = useMemo(() => params, [paramsKey]);
    const queryKey = useMemo(() => `${sql}\0${paramsKey}`, [paramsKey, sql]);
    const mapRowsRef = useRef(mapRows);
    const mappedDataRef = useRef<
      | {
          queryKey: string;
          rows: unknown[] | undefined;
          data: TData | undefined;
        }
      | undefined
    >(undefined);

    mapRowsRef.current = mapRows;
    const query = useMemo(
      () =>
        enabled
          ? createQueryHandle(() => getSharedQuery(queryKey, sql, stableParams))
          : DISABLED_QUERY,
      [enabled, queryKey, sql, stableParams],
    );
    const snapshot = useSyncExternalStore(
      query.subscribe,
      query.getSnapshot,
      query.getSnapshot,
    );
    const mappedQueryKey = enabled ? queryKey : "";

    if (
      mappedDataRef.current?.queryKey !== mappedQueryKey ||
      mappedDataRef.current.rows !== snapshot.rows
    ) {
      const rows = snapshot.rows as TRow[] | undefined;
      mappedDataRef.current = {
        queryKey: mappedQueryKey,
        rows: snapshot.rows,
        data:
          rows === undefined
            ? undefined
            : mapRowsRef.current
              ? mapRowsRef.current(rows)
              : (rows as TData),
      };
    }

    return {
      data: mappedDataRef.current.data,
      isLoading: snapshot.isLoading,
      error: snapshot.error,
    };
  };
}

export function createUseDrizzleLiveQuery(client: LiveQueryClient) {
  const useLiveQuery = createUseLiveQuery(client);

  return function useDrizzleLiveQuery<TRow, TData = TRow[]>(
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
  };
}
