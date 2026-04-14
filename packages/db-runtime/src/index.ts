export type Row = Record<string, unknown>;

export type QueryEvent<T = Row> =
  | { event: "result"; data: T[] }
  | { event: "error"; data: string };

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
