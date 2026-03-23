import { Channel, invoke } from "@tauri-apps/api/core";

// ── Types ────────────────────────────────────────────────────────────────────

export type QueryEvent =
  | { event: "result"; data: Record<string, unknown>[] }
  | { event: "error"; data: string };

export interface SubscribeOptions<T = Record<string, unknown>> {
  onData: (rows: T[]) => void;
  onError?: (error: string) => void;
}

// ── Commands ─────────────────────────────────────────────────────────────────

export async function subscribe<T = Record<string, unknown>>(
  sql: string,
  params: unknown[],
  options: SubscribeOptions<T>,
): Promise<() => void> {
  const channel = new Channel<QueryEvent>();

  channel.onmessage = (event: QueryEvent) => {
    if (event.event === "result") {
      options.onData(event.data as T[]);
    } else if (event.event === "error") {
      options.onError?.(event.data);
    }
  };

  const subscriptionId: string = await invoke("plugin:reactive-db|subscribe", {
    sql,
    params,
    onEvent: channel,
  });

  return () => {
    invoke("plugin:reactive-db|unsubscribe", { subscriptionId }).catch(
      () => {},
    );
  };
}

export async function execute(
  sql: string,
  params: unknown[] = [],
): Promise<Record<string, unknown>[]> {
  return invoke("plugin:reactive-db|execute", { sql, params });
}
