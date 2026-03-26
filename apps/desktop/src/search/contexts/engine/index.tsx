import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

import { commands as tantivy } from "@hypr/plugin-tantivy";

import { buildTantivyFilters } from "./filters";
import { indexHumans, indexOrganizations, indexSessions } from "./indexing";
import {
  createHumanListener,
  createOrganizationListener,
  createSessionListener,
} from "./listeners";
import type { SearchEntityType, SearchFilters, SearchHit } from "./types";
import { normalizeQuery } from "./utils";

import { type Store as MainStore } from "~/store/tinybase/store/main";

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
  store,
}: {
  children: React.ReactNode;
  store?: MainStore;
}) {
  const [isIndexing, setIsIndexing] = useState(true);
  const listenerIds = useRef<string[]>([]);

  useEffect(() => {
    if (!store) {
      return;
    }

    const initializeIndex = async () => {
      setIsIndexing(true);

      try {
        await indexSessions(store);
        await indexHumans(store);
        await indexOrganizations(store);

        const listener1 = store.addRowListener(
          "sessions",
          null,
          createSessionListener(),
        );
        const listener2 = store.addRowListener(
          "humans",
          null,
          createHumanListener(),
        );
        const listener3 = store.addRowListener(
          "organizations",
          null,
          createOrganizationListener(),
        );

        listenerIds.current = [listener1, listener2, listener3];
      } catch (error) {
        console.error("Failed to create search index:", error);
      } finally {
        setIsIndexing(false);
      }
    };

    void initializeIndex();

    return () => {
      listenerIds.current.forEach((id) => {
        store.delListener(id);
      });
      listenerIds.current = [];
    };
  }, [store]);

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
