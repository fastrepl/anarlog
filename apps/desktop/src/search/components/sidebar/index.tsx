import { SearchXIcon } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import { forwardRef } from "react";

import { cn } from "@hypr/utils";

import { SearchResultGroup } from "./group";

import { type GroupedSearchResults, useSearch } from "~/search/contexts/ui";

export function SearchResults() {
  const { results, query, setQuery, selectedIndex } = useSearch();
  const containerRef = useRef<HTMLDivElement>(null);

  const empty = !query || !results || results.totalResults === 0;

  const flatResults = useMemo(() => {
    if (!results) return [];
    return results.groups.flatMap((g) => g.results);
  }, [results]);

  const selectedId =
    selectedIndex >= 0 && selectedIndex < flatResults.length
      ? flatResults[selectedIndex].id
      : null;

  useEffect(() => {
    if (!selectedId || !containerRef.current) return;
    const el = containerRef.current.querySelector(
      `[data-result-id="${selectedId}"]`,
    );
    if (el) {
      el.scrollIntoView({ block: "nearest" });
    }
  }, [selectedId]);

  return (
    <div className={cn(["h-full rounded-xl bg-neutral-50"])}>
      {empty ? (
        <SearchNoResults query={query} setQuery={setQuery} />
      ) : (
        <SearchYesResults
          ref={containerRef}
          results={results}
          selectedId={selectedId}
        />
      )}
    </div>
  );
}

const SearchYesResults = forwardRef<
  HTMLDivElement,
  { results: GroupedSearchResults; selectedId: string | null }
>(({ results, selectedId }, ref) => {
  return (
    <div ref={ref} className="scrollbar-hide h-full overflow-y-auto">
      {results.groups.map((group) => (
        <SearchResultGroup
          key={group.key}
          group={group}
          selectedId={selectedId}
        />
      ))}
    </div>
  );
});

function SearchNoResults({
  query,
}: {
  query: string;
  setQuery: (query: string) => void;
}) {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="max-w-xs px-4 text-center">
        <div className="mb-3 flex justify-center">
          <SearchXIcon className="h-10 w-10 text-neutral-300" />
        </div>
        <p className="text-sm font-medium text-neutral-700">
          No results found for "{query}"
        </p>
        <p className="mt-2 text-xs leading-relaxed text-neutral-500 underline">
          Help us improve search
        </p>
      </div>
    </div>
  );
}
