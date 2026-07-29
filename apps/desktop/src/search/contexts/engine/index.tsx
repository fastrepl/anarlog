import { createContext, useCallback, useContext } from "react";

import { commands as tantivy } from "@anlg/plugin-tantivy";

import { buildTantivyFilters } from "./filters";
import type { SearchEntityType, SearchFilters, SearchHit } from "./types";
import { normalizeQuery } from "./utils";

import { trackAnalyticsEvent } from "~/analytics";

export type {
  SearchDocument,
  SearchEntityType,
  SearchFilters,
  SearchHit,
} from "./types";

const SearchEngineContext = createContext<{
  search: (
    query: string,
    filters?: SearchFilters | null,
  ) => Promise<SearchHit[]>;
} | null>(null);

export function SearchEngineProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const search = useCallback(
    async (
      query: string,
      filters: SearchFilters | null = null,
    ): Promise<SearchHit[]> => {
      const normalizedQuery = normalizeQuery(query);
      const tantivyFilters = buildTantivyFilters(filters);
      const filterCount = filters?.created_at ? 1 : 0;
      const startedAt = performance.now();

      try {
        const result = await tantivy.search({
          query: normalizedQuery,
          filters: tantivyFilters,
        });

        if (result.status === "error") {
          console.error("Search failed:", result.error);
          trackAnalyticsEvent("search_performed", {
            outcome: "failed",
            latency_ms: Math.round(performance.now() - startedAt),
            filter_count: filterCount,
          });
          return [];
        }

        const hits = result.data.hits.map((hit) => ({
          score: hit.score,
          document: {
            id: hit.document.id,
            type: hit.document.doc_type as SearchEntityType,
            title: hit.document.title,
            content: hit.document.content,
            created_at: hit.document.created_at,
          },
        }));
        trackAnalyticsEvent("search_performed", {
          outcome: "succeeded",
          result_count: hits.length,
          latency_ms: Math.round(performance.now() - startedAt),
          filter_count: filterCount,
          entity_types: [
            ...new Set(hits.map((hit) => hit.document.type)),
          ].sort(),
        });
        return hits;
      } catch (error) {
        console.error("Search failed:", error);
        trackAnalyticsEvent("search_performed", {
          outcome: "failed",
          latency_ms: Math.round(performance.now() - startedAt),
          filter_count: filterCount,
        });
        return [];
      }
    },
    [],
  );

  const value = {
    search,
  };

  return (
    <SearchEngineContext.Provider value={value}>
      {children}
    </SearchEngineContext.Provider>
  );
}

export function useSearchEngine() {
  const context = useContext(SearchEngineContext);
  if (!context) {
    throw new Error("useSearchEngine must be used within SearchEngineProvider");
  }
  return context;
}
