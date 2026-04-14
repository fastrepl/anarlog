export type Row = Record<string, unknown>;
export type ProxyQueryMethod = "run" | "all" | "get" | "values";
export type ProxyQueryResult = { rows: unknown[] };

export type QueryEvent<T = Row> =
  | { event: "result"; data: T[] }
  | { event: "error"; data: string };

export type DrizzleProxyClient = {
  executeProxy(
    sql: string,
    params: unknown[],
    method: ProxyQueryMethod,
  ): Promise<ProxyQueryResult>;
};

export type LiveQueryClient = {
  execute<T = Row>(sql: string, params?: unknown[]): Promise<T[]>;
  subscribe<T = Row>(
    sql: string,
    params: unknown[],
    options: {
      onData: (rows: T[]) => void;
      onError?: (error: string) => void;
    },
  ): Promise<() => void>;
};
