import { drizzle } from "drizzle-orm/sqlite-proxy";

import { execute } from "@hypr/plugin-db";

import * as schema from "./schema";

export const db = drizzle(
  async (sql, params, method) => {
    try {
      if (method === "run") {
        await execute(sql, params);
        return { rows: [] };
      }

      const rows = await execute(sql, params);
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

export * from "./schema";
export { eq, and, or, desc, asc, sql, count, max, ne } from "drizzle-orm";
