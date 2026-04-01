import { tool } from "ai";
import { z } from "zod";

import type { ToolDependencies } from "./types";

export const buildSearchContactsTool = (deps: ToolDependencies) =>
  tool({
    description:
      "Search contacts and return names, emails, roles, and organizations.",
    inputSchema: z.object({
      query: z.string().describe("Search query for contacts"),
      limit: z
        .number()
        .int()
        .min(1)
        .max(20)
        .optional()
        .describe("Maximum number of contacts to return"),
    }),
    execute: async (params: { query: string; limit?: number }) => {
      const limit = params.limit ?? 8;
      const results = await deps.getContactSearchResults(params.query, limit);

      return {
        query: params.query,
        results,
      };
    },
  });
