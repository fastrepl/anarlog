import { tool } from "ai";
import { z } from "zod";

import type { ToolDependencies } from "./types";

import { searchFiltersSchema } from "~/search/contexts/engine/types";

export const buildSearchSessionsTool = (deps: ToolDependencies) =>
  tool({
    description: `
Search for sessions (meeting notes) using query and filters.
Returns relevant sessions with their content.
`.trim(),
    inputSchema: z.object({
      query: z.string().describe("The search query to find relevant sessions"),
      filters: searchFiltersSchema
        .optional()
        .describe("Optional filters for the search query"),
      limit: z
        .number()
        .int()
        .min(1)
        .max(10)
        .optional()
        .describe("Maximum number of sessions to return"),
    }),
    execute: async (params: {
      query: string;
      filters?: z.infer<typeof searchFiltersSchema>;
      limit?: number;
    }) => {
      const hits = await deps.search(params.query, params.filters || null);
      const sessionHits = hits.filter((hit) => hit.document.type === "session");
      const limit = params.limit ?? 5;

      const results = sessionHits.slice(0, limit).map((hit) => ({
        id: hit.document.id,
        title: hit.document.title,
        excerpt: hit.document.content.slice(0, 180),
        score: hit.score,
        created_at: hit.document.created_at,
      }));

      return { results };
    },
  });
