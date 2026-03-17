import { Loader2Icon, SearchIcon, XIcon } from "lucide-react";
import { useEffect } from "react";

import { Kbd } from "@hypr/ui/components/ui/kbd";
import { useCmdKeyPressed } from "@hypr/ui/hooks/use-cmd-key-pressed";
import { cn } from "@hypr/utils";

import { useSearchKeyboard } from "./use-search-keyboard";

import { useSearch } from "~/search/contexts/ui";

export function SidebarSearchInput() {
  const { query, setQuery, inputRef, setFocusImpl, isSearching, isIndexing } =
    useSearch();
  const isCmdPressed = useCmdKeyPressed();
  const { onKeyDown } = useSearchKeyboard();

  useEffect(() => {
    setFocusImpl(() => {
      inputRef.current?.focus();
    });
  }, [setFocusImpl, inputRef]);

  const showLoading = isSearching || isIndexing;
  const showShortcut = isCmdPressed && !query;

  return (
    <div className="relative flex h-8 shrink-0 items-center px-2">
      {showLoading ? (
        <Loader2Icon
          className={cn([
            "absolute left-5 h-4 w-4 animate-spin text-neutral-400",
          ])}
        />
      ) : (
        <SearchIcon
          className={cn(["absolute left-5 h-4 w-4 text-neutral-400"])}
        />
      )}
      <input
        ref={inputRef}
        type="text"
        placeholder="Search anything..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKeyDown}
        className={cn([
          "text-sm placeholder:text-sm placeholder:text-neutral-400",
          "h-full w-full pl-8",
          query ? "pr-8" : showShortcut ? "pr-14" : "pr-4",
          "rounded-lg border border-neutral-200 bg-neutral-200/50",
          "focus:bg-neutral-200 focus:outline-hidden",
        ])}
      />
      {query && (
        <button
          onClick={() => setQuery("")}
          className={cn([
            "absolute right-5",
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
        <div className="absolute top-1 right-4">
          <Kbd>⌘ K</Kbd>
        </div>
      )}
    </div>
  );
}
