import type { SearchFilters as TantivySearchFilters } from "@hypr/plugin-tantivy";

import type { SearchFilters } from "./types";

export function buildTantivyFilters(
  filters: SearchFilters | null,
): TantivySearchFilters | undefined {
  if (!filters || !filters.created_at) {
    return undefined;
  }

  return {
    created_at: {
      gte: filters.created_at.gte ?? null,
      lte: filters.created_at.lte ?? null,
      gt: filters.created_at.gt ?? null,
      lt: filters.created_at.lt ?? null,
      eq: filters.created_at.eq ?? null,
    },
    doc_type: null,
    facet: null,
  };
}
