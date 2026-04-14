import { drizzle } from "drizzle-orm/sqlite-proxy";

import type { DrizzleProxyClient } from "@hypr/db-runtime";

import * as schema from "./schema";

export function createDb(client: DrizzleProxyClient) {
  return drizzle(
    async (sql, params, method) => {
      try {
        return await client.executeProxy(sql, params, method);
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
