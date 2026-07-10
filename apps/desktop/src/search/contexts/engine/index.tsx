import { createContext, useCallback, useContext, useState } from "react";

import { commands as tantivy } from "@hypr/plugin-tantivy";

import { buildTantivyFilters } from "./filters";
import { createSearchIndexSync } from "./indexing";
import type { SearchEntityType, SearchFilters, SearchHit } from "./types";
import { normalizeQuery } from "./utils";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

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
  isIndexing: boolean;
} | null>(null);

export function SearchEngineProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [isIndexing, setIsIndexing] = useState(true);

  useMountEffect(() => {
    const sync = createSearchIndexSync();
    let disposed = false;
    void sync
      .start()
      .catch((error) => {
        console.error("Failed to create search index:", error);
      })
      .finally(() => {
        if (!disposed) setIsIndexing(false);
      });

    return () => {
      disposed = true;
      void sync.stop();
    };
  });

  const search = useCallback(
    async (
      query: string,
      filters: SearchFilters | null = null,
    ): Promise<SearchHit[]> => {
      const normalizedQuery = normalizeQuery(query);
      const tantivyFilters = buildTantivyFilters(filters);

      try {
        const result = await tantivy.search({
          query: normalizedQuery,
          filters: tantivyFilters,
        });

        if (result.status === "error") {
          console.error("Search failed:", result.error);
          return [];
        }

        return result.data.hits.map((hit) => ({
          score: hit.score,
          document: {
            id: hit.document.id,
            type: hit.document.doc_type as SearchEntityType,
            title: hit.document.title,
            content: hit.document.content,
            created_at: hit.document.created_at,
          },
        }));
      } catch (error) {
        console.error("Search failed:", error);
        return [];
      }
    },
    [],
  );

  const value = {
    search,
    isIndexing,
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
