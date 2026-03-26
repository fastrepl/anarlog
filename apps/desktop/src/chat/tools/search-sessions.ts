import { tool } from "ai";
import { z } from "zod";

import type { ToolDependencies } from "./types";

import type { SearchFilters } from "~/search/contexts/engine/types";
import { getSessionSearchTimestamp } from "~/search/contexts/engine/utils";

const gteSchema = z
  .number()
  .optional()
  .describe("Include sessions on or after this Unix timestamp in milliseconds");
const lteSchema = z
  .number()
  .optional()
  .describe(
    "Include sessions on or before this Unix timestamp in milliseconds",
  );
const gtSchema = z
  .number()
  .optional()
  .describe("Include sessions after this Unix timestamp in milliseconds");
const ltSchema = z
  .number()
  .optional()
  .describe("Include sessions before this Unix timestamp in milliseconds");
const eqSchema = z
  .number()
  .optional()
  .describe(
    "Include only sessions at this exact Unix timestamp in milliseconds",
  );

const absoluteCreatedAtFilterSchema = z
  .object({
    kind: z.literal("absolute"),
    gte: gteSchema,
    lte: lteSchema,
    gt: gtSchema,
    lt: ltSchema,
    eq: eqSchema,
  })
  .describe(
    "Absolute timestamp bounds using Unix milliseconds. Use this only when you already know the exact timestamps.",
  );

const relativeCreatedAtFilterSchema = z
  .object({
    kind: z.literal("relative"),
    recent_days: z
      .number()
      .int()
      .min(1)
      .max(365)
      .describe(
        "Use for requests like 'last N days'. Includes today and counts backward in local time.",
      ),
  })
  .describe(
    "Relative date filter. Prefer this over absolute timestamps for natural-language time ranges.",
  );

const createdAtFilterSchema = z
  .union([absoluteCreatedAtFilterSchema, relativeCreatedAtFilterSchema])
  .describe(
    "Date filter for sessions. Uses event started_at for event-backed sessions, otherwise session created_at.",
  );

const searchSessionsFiltersSchema = z
  .object({
    created_at: createdAtFilterSchema.optional(),
  })
  .optional()
  .describe("Optional session filters");

type AbsoluteCreatedAtFilter = z.infer<typeof absoluteCreatedAtFilterSchema>;
type SearchSessionsFiltersInput = z.infer<typeof searchSessionsFiltersSchema>;

function getRecentDaysFilter(
  days: number,
): NonNullable<SearchFilters["created_at"]> {
  const now = new Date();
  const startOfToday = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
  ).getTime();
  const endOfToday = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
    23,
    59,
    59,
    999,
  ).getTime();

  return {
    gte: startOfToday - Math.max(days - 1, 0) * 24 * 60 * 60 * 1000,
    lte: endOfToday,
  };
}

export const buildSearchSessionsTool = (deps: ToolDependencies) =>
  tool({
    description: `
Search for sessions (meeting notes) using query and filters.
Use filters.created_at.kind="relative" with recent_days for natural-language date ranges.
Use an empty query string when the user only wants sessions by date/time filter.
Returns relevant sessions with their content.
`.trim(),
    inputSchema: z.object({
      query: z
        .string()
        .optional()
        .describe(
          "Optional text query for finding relevant sessions. Omit this when filtering only by date/time.",
        ),
      filters: searchSessionsFiltersSchema,
      limit: z
        .number()
        .int()
        .min(0)
        .max(10)
        .optional()
        .describe("Maximum number of sessions to return"),
    }),
    execute: async (params: {
      query?: string;
      filters?: SearchSessionsFiltersInput;
      limit?: number;
    }) => {
      const store = deps.getStore();
      const query = params.query ?? "";
      const createdAtFilter = params.filters?.created_at;
      const effectiveFilters: SearchFilters | null = createdAtFilter
        ? {
            created_at:
              createdAtFilter.kind === "absolute"
                ? (({ gte, lte, gt, lt, eq }: AbsoluteCreatedAtFilter) => ({
                    gte,
                    lte,
                    gt,
                    lt,
                    eq,
                  }))(createdAtFilter)
                : getRecentDaysFilter(createdAtFilter.recent_days),
          }
        : null;

      const hits = await deps.search(query, effectiveFilters);
      const sessionHits = hits.filter((hit) => hit.document.type === "session");
      const limit = params.limit ?? 5;
      const results = sessionHits.slice(0, limit).map((hit) => {
        const sessionRow = store?.getRow("sessions", hit.document.id);

        return {
          id: hit.document.id,
          title: hit.document.title,
          excerpt: hit.document.content.slice(0, 180),
          score: hit.score,
          created_at: sessionRow
            ? getSessionSearchTimestamp(sessionRow)
            : hit.document.created_at,
        };
      });

      return { results };
    },
  });
