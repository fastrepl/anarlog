import { Loader2Icon, SearchIcon, XIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { cn } from "@hypr/utils";

import { ResultItem } from "./result-item";

import { useSearchEngine } from "~/search/contexts/engine";
import {
  type GroupedSearchResults,
  groupSearchResults,
  type SearchEntityType,
} from "~/search/contexts/ui";

const FILTER_OPTIONS: { type: SearchEntityType; label: string }[] = [
  { type: "session", label: "Meeting note" },
  { type: "human", label: "Person" },
  { type: "organization", label: "Organization" },
];

type DatePreset = "today" | "week" | "month";

const DATE_PRESETS: { key: DatePreset; label: string }[] = [
  { key: "today", label: "Today" },
  { key: "week", label: "This week" },
  { key: "month", label: "This month" },
];

function getDateRange(preset: DatePreset): { gte: number; lte: number } {
  const now = new Date();
  const endOfDay = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
    23,
    59,
    59,
    999,
  );
  const lte = endOfDay.getTime();

  switch (preset) {
    case "today": {
      const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
      return { gte: start.getTime(), lte };
    }
    case "week": {
      const start = new Date(
        now.getFullYear(),
        now.getMonth(),
        now.getDate() - now.getDay(),
      );
      return { gte: start.getTime(), lte };
    }
    case "month": {
      const start = new Date(now.getFullYear(), now.getMonth(), 1);
      return { gte: start.getTime(), lte };
    }
  }
}

interface AdvancedSearchViewProps {
  initialQuery?: string;
  selectedTypes: string[] | null;
  setSelectedTypes: (types: string[] | null) => void;
  onResultClick: (type: string, id: string) => void;
}

export function AdvancedSearchView({
  initialQuery,
  selectedTypes,
  setSelectedTypes,
  onResultClick,
}: AdvancedSearchViewProps) {
  const { search, isIndexing } = useSearchEngine();
  const [localQuery, setLocalQuery] = useState(initialQuery ?? "");
  const [query, setQuery] = useState(initialQuery ?? "");
  const [results, setResults] = useState<GroupedSearchResults | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [activeDatePreset, setActiveDatePreset] = useState<DatePreset | null>(
    null,
  );
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const timer = setTimeout(() => {
      setQuery(localQuery);
    }, 50);
    return () => clearTimeout(timer);
  }, [localQuery]);

  const dateFilter = useMemo(
    () =>
      activeDatePreset ? { created_at: getDateRange(activeDatePreset) } : null,
    [activeDatePreset],
  );

  useEffect(() => {
    if (query.trim().length < 1) {
      setResults(null);
      setIsSearching(false);
      return;
    }

    let cancelled = false;
    setIsSearching(true);

    search(query, dateFilter).then((hits) => {
      if (!cancelled) {
        setResults(groupSearchResults(hits, query.trim()));
        setIsSearching(false);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [query, search, dateFilter]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const toggleFilter = useCallback(
    (type: SearchEntityType) => {
      if (!selectedTypes) {
        setSelectedTypes([type]);
      } else if (selectedTypes.includes(type)) {
        const newTypes = selectedTypes.filter((t) => t !== type);
        setSelectedTypes(newTypes.length > 0 ? newTypes : null);
      } else {
        setSelectedTypes([...selectedTypes, type]);
      }
    },
    [selectedTypes, setSelectedTypes],
  );

  const toggleDatePreset = useCallback((key: DatePreset) => {
    setActiveDatePreset((prev) => (prev === key ? null : key));
  }, []);

  const filteredResults = useMemo(() => {
    if (!results || !selectedTypes || selectedTypes.length === 0) {
      return results;
    }
    return {
      ...results,
      groups: results.groups.filter((group) =>
        selectedTypes.includes(group.type),
      ),
      totalResults: results.groups
        .filter((group) => selectedTypes.includes(group.type))
        .reduce((acc, group) => acc + group.totalCount, 0),
    };
  }, [results, selectedTypes]);

  const [selectedIndex, setSelectedIndex] = useState(-1);

  const flatResults = useMemo(() => {
    if (!filteredResults) return [];
    return filteredResults.groups.flatMap((g) => g.results);
  }, [filteredResults]);

  useEffect(() => {
    setSelectedIndex(-1);
  }, [filteredResults]);

  const selectedId =
    selectedIndex >= 0 && selectedIndex < flatResults.length
      ? flatResults[selectedIndex].id
      : null;

  const resultsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!selectedId || !resultsRef.current) return;
    const el = resultsRef.current.querySelector(
      `[data-result-id="${selectedId}"]`,
    );
    if (el) {
      el.scrollIntoView({ block: "nearest" });
    }
  }, [selectedId]);

  const showLoading = isSearching || isIndexing;
  const hasQuery = query.trim().length > 0;
  const hasResults = filteredResults && filteredResults.totalResults > 0;

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-neutral-200 py-1 pr-1">
        <div className="relative">
          {showLoading ? (
            <Loader2Icon className="absolute top-1/2 left-[14px] h-4 w-4 -translate-y-1/2 animate-spin text-neutral-400" />
          ) : (
            <SearchIcon className="absolute top-1/2 left-[14px] h-4 w-4 -translate-y-1/2 text-neutral-400" />
          )}
          <input
            ref={inputRef}
            type="text"
            placeholder="Try 'budget', '@john', or '#design'"
            value={localQuery}
            onChange={(e) => setLocalQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                if (localQuery.trim()) {
                  setLocalQuery("");
                } else {
                  e.currentTarget.blur();
                }
              }
              if (e.key === "ArrowDown" && flatResults.length > 0) {
                e.preventDefault();
                setSelectedIndex(
                  Math.min(selectedIndex + 1, flatResults.length - 1),
                );
              }
              if (e.key === "ArrowUp" && flatResults.length > 0) {
                e.preventDefault();
                setSelectedIndex(Math.max(selectedIndex - 1, -1));
              }
              if (
                e.key === "Enter" &&
                selectedIndex >= 0 &&
                selectedIndex < flatResults.length
              ) {
                e.preventDefault();
                const item = flatResults[selectedIndex];
                onResultClick(item.type, item.id);
              }
            }}
            className={cn([
              "w-full py-2 pr-8 pl-[38px]",
              "text-base placeholder:text-neutral-400",
              "bg-transparent",
              "border-none",
              "focus:outline-none",
              "transition-all",
            ])}
          />
          {localQuery && (
            <button
              onClick={() => setLocalQuery("")}
              className="absolute top-1/2 right-2 -translate-y-1/2 text-neutral-400 hover:text-neutral-600"
            >
              <XIcon className="h-5 w-5" />
            </button>
          )}
        </div>
      </div>

      <div className="border-b border-neutral-200 py-2 pr-3 pl-[14px]">
        <div className="flex flex-wrap gap-2">
          {FILTER_OPTIONS.map((option) => {
            const isActive = selectedTypes?.includes(option.type);
            return (
              <Badge
                key={option.type}
                variant="outline"
                className={cn([
                  "cursor-pointer transition-all",
                  isActive
                    ? "border-stone-600 bg-stone-600 text-white hover:bg-stone-700"
                    : "border-neutral-200 bg-white text-neutral-600 hover:bg-neutral-100",
                ])}
                onClick={() => toggleFilter(option.type)}
              >
                {option.label}
              </Badge>
            );
          })}
          {DATE_PRESETS.map((preset) => {
            const isActive = activeDatePreset === preset.key;
            return (
              <Badge
                key={preset.key}
                variant="outline"
                className={cn([
                  "cursor-pointer transition-all",
                  isActive
                    ? "border-stone-600 bg-stone-600 text-white hover:bg-stone-700"
                    : "border-neutral-200 bg-white text-neutral-600 hover:bg-neutral-100",
                ])}
                onClick={() => toggleDatePreset(preset.key)}
              >
                {preset.label}
              </Badge>
            );
          })}
        </div>
      </div>

      <div ref={resultsRef} className="flex-1 overflow-y-auto">
        {!hasQuery ? (
          <SuggestionsView
            results={filteredResults}
            onResultClick={onResultClick}
            selectedId={selectedId}
          />
        ) : hasResults ? (
          <SearchResultsView
            results={filteredResults!}
            onResultClick={onResultClick}
            selectedId={selectedId}
          />
        ) : (
          <NoResultsView query={query} />
        )}
      </div>
    </div>
  );
}

function SuggestionsView({
  results,
  onResultClick,
  selectedId,
}: {
  results: GroupedSearchResults | null;
  onResultClick: (type: string, id: string) => void;
  selectedId: string | null;
}) {
  return (
    <div className="pt-3 pr-3 pl-[14px]">
      {results && results.totalResults > 0 ? (
        <div className="space-y-1">
          {results.groups
            .slice(0, 3)
            .flatMap((group) =>
              group.results
                .slice(0, 5)
                .map((result) => (
                  <ResultItem
                    key={result.id}
                    result={result}
                    onClick={() => onResultClick(result.type, result.id)}
                    isSelected={result.id === selectedId}
                  />
                )),
            )}
        </div>
      ) : (
        <div className="py-12 text-center text-neutral-400">
          <p>Start typing to search</p>
          <p className="mt-1 text-sm">or browse your recent notes</p>
        </div>
      )}
    </div>
  );
}

function SearchResultsView({
  results,
  onResultClick,
  selectedId,
}: {
  results: GroupedSearchResults;
  onResultClick: (type: string, id: string) => void;
  selectedId: string | null;
}) {
  return (
    <div className="pt-3 pr-3 pl-[14px]">
      {results.groups.map((group) => (
        <div key={group.key} className="mb-6">
          <h3 className="mb-3 text-sm font-semibold text-neutral-900">
            {group.title}
          </h3>
          <div className="space-y-1">
            {group.results.map((result) => (
              <ResultItem
                key={result.id}
                result={result}
                onClick={() => onResultClick(result.type, result.id)}
                isSelected={result.id === selectedId}
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function NoResultsView({ query }: { query: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center py-12">
      <SearchIcon className="mb-4 h-12 w-12 text-neutral-200" />
      <p className="font-medium text-neutral-600">No results found</p>
      <p className="mt-1 text-sm text-neutral-400">No matches for "{query}"</p>
    </div>
  );
}
