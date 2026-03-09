import { Loader2Icon, SearchIcon, XIcon } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Kbd } from "@hypr/ui/components/ui/kbd";
import { useCmdKeyPressed } from "@hypr/ui/hooks/use-cmd-key-pressed";
import { cn } from "@hypr/utils";

import { useSearch } from "~/search/contexts/ui";
import { useTabs } from "~/store/zustand/tabs";

export function Search({
  onManualExpandChange,
}: {
  onManualExpandChange?: (isManuallyExpanded: boolean) => void;
}) {
  const { focus, setFocusImpl, inputRef } = useSearch();
  const [isManuallyExpanded, setIsManuallyExpanded] = useState(false);

  useEffect(() => {
    onManualExpandChange?.(isManuallyExpanded);
  }, [isManuallyExpanded, onManualExpandChange]);

  useEffect(() => {
    if (!isManuallyExpanded) {
      setFocusImpl(() => {
        setIsManuallyExpanded(true);
        setTimeout(() => inputRef.current?.focus(), 100);
      });
    } else {
      setFocusImpl(() => {
        inputRef.current?.focus();
      });
    }
  }, [isManuallyExpanded, setFocusImpl, inputRef]);

  const handleCollapsedClick = () => {
    focus();
  };

  const handleExpandedBlur = () => {
    setIsManuallyExpanded(false);
  };

  if (isManuallyExpanded) {
    return <ExpandedSearch onBlur={handleExpandedBlur} />;
  }

  return <CollapsedSearch onClick={handleCollapsedClick} />;
}

function CollapsedSearch({ onClick }: { onClick: () => void }) {
  const { isSearching, isIndexing } = useSearch();
  const showLoading = isSearching || isIndexing;

  return (
    <Button
      onClick={onClick}
      size="icon"
      variant="ghost"
      className="text-neutral-400"
    >
      {showLoading ? (
        <Loader2Icon className="size-4 animate-spin" />
      ) : (
        <SearchIcon className="size-4" />
      )}
    </Button>
  );
}

function ExpandedSearch({ onBlur }: { onBlur?: () => void }) {
  const {
    query,
    setQuery,
    isSearching,
    isIndexing,
    inputRef,
    results,
    selectedIndex,
    setSelectedIndex,
  } = useSearch();
  const [isFocused, setIsFocused] = useState(false);
  const isCmdPressed = useCmdKeyPressed();
  const openNew = useTabs((state) => state.openNew);

  const flatResults = useMemo(() => {
    if (!results) return [];
    return results.groups.flatMap((g) => g.results);
  }, [results]);

  const showLoading = isSearching || isIndexing;
  const showShortcut = isCmdPressed && !query;
  const hasResults = results && results.totalResults > 0;
  const resultCount = results?.totalResults ?? 0;

  const width = isFocused ? "w-[250px]" : "w-[180px]";

  return (
    <div
      data-tauri-drag-region
      className={cn([
        "flex h-full items-center transition-all duration-300",
        width,
      ])}
    >
      <div className="relative flex h-full w-full items-center">
        {showLoading ? (
          <Loader2Icon
            className={cn([
              "absolute left-3 h-4 w-4 animate-spin text-neutral-400",
            ])}
          />
        ) : (
          <SearchIcon
            className={cn(["absolute left-3 h-4 w-4 text-neutral-400"])}
          />
        )}
        <input
          ref={inputRef}
          type="text"
          placeholder="Search anything..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              if (query.trim()) {
                setQuery("");
                setSelectedIndex(-1);
              } else {
                e.currentTarget.blur();
              }
            }
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && query.trim()) {
              e.preventDefault();
              openNew({
                type: "search",
                state: {
                  selectedTypes: null,
                  initialQuery: query.trim(),
                },
              });
              setQuery("");
              e.currentTarget.blur();
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
              !e.metaKey &&
              !e.ctrlKey &&
              selectedIndex >= 0 &&
              selectedIndex < flatResults.length
            ) {
              e.preventDefault();
              const item = flatResults[selectedIndex];
              if (item.type === "session") {
                openNew({ type: "sessions", id: item.id });
              } else if (item.type === "human") {
                openNew({
                  type: "contacts",
                  state: {
                    selected: { type: "person", id: item.id },
                  },
                });
              } else if (item.type === "organization") {
                openNew({
                  type: "contacts",
                  state: {
                    selected: { type: "organization", id: item.id },
                  },
                });
              }
              e.currentTarget.blur();
            }
          }}
          onFocus={() => {
            setIsFocused(true);
          }}
          onBlur={() => {
            setIsFocused(false);
            onBlur?.();
          }}
          className={cn([
            "text-sm placeholder:text-sm placeholder:text-neutral-400",
            "h-full w-full pl-9",
            query
              ? hasResults
                ? "pr-16"
                : "pr-9"
              : showShortcut
                ? "pr-14"
                : "pr-4",
            "rounded-xl bg-neutral-100",
            "focus:bg-neutral-200 focus:outline-hidden",
          ])}
        />
        {hasResults && query && (
          <div
            className={cn([
              "absolute right-9",
              "px-2 py-0.5",
              "rounded-full bg-neutral-400",
              "text-xs font-semibold text-white",
              "pointer-events-none",
            ])}
          >
            {resultCount}
          </div>
        )}
        {query && (
          <button
            onClick={() => setQuery("")}
            className={cn([
              "absolute right-3",
              "h-4 w-4",
              "text-neutral-400 hover:text-neutral-600",
              "transition-colors",
            ])}
            aria-label="Clear search"
          >
            <XIcon className="h-4 w-4" />
          </button>
        )}
        {showShortcut && (
          <div className="absolute top-1 right-2">
            <Kbd>⌘ K</Kbd>
          </div>
        )}
      </div>
    </div>
  );
}
