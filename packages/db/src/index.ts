import { drizzle } from "drizzle-orm/sqlite-proxy";

import type { LiveQueryClient } from "@hypr/db-runtime";

import * as schema from "./schema";

export function createDb(client: Pick<LiveQueryClient, "execute">) {
  return drizzle(
    async (sql, params, method) => {
      try {
        if (method === "run") {
          await client.execute(sql, params);
          return { rows: [] };
        }

        const rows = await client.execute(sql, params);
        const mapped = rows.map((row) =>
          Object.values(row as Record<string, unknown>),
        );

        if (method === "get") {
          return { rows: mapped[0] ?? [] };
        }

        return { rows: mapped };
      } catch (error) {
        console.error("[drizzle-proxy]", method, sql, error);
        throw error;
      }
    },
    { schema },
  );
}

export * from "./schema";
export { eq, and, or, desc, asc, sql, count, max, ne } from "drizzle-orm";
