import { ChevronDownIcon } from "lucide-react";
import { useState } from "react";

import { cn } from "@hypr/utils";

import { SearchResultItem } from "./item";

import { type SearchGroup } from "~/search/contexts/ui";

const ITEMS_PER_PAGE = 3;
const LOAD_MORE_STEP = 5;

export function SearchResultGroup({
  group,
  selectedId,
}: {
  group: SearchGroup;
  selectedId: string | null;
}) {
  const [visibleCount, setVisibleCount] = useState(ITEMS_PER_PAGE);

  if (group.totalCount === 0) {
    return null;
  }

  const visibleResults = group.results.slice(0, visibleCount);
  const hasMore = group.totalCount > visibleCount;

  return (
    <div className={cn(["mb-4"])}>
      <div
        className={cn(["sticky top-0 z-10", "bg-neutral-50 py-1 pr-1 pl-3"])}
      >
        <h3 className={cn(["text-base font-bold text-neutral-900"])}>
          {group.title}
        </h3>
      </div>
      <div>
        {visibleResults.map((result) => (
          <SearchResultItem
            key={result.id}
            result={result}
            isSelected={result.id === selectedId}
          />
        ))}
      </div>
      {hasMore && (
        <button
          onClick={() => setVisibleCount((prev) => prev + LOAD_MORE_STEP)}
          className={cn([
            "mt-1 w-full px-3 py-2",
            "flex items-center justify-center gap-2",
            "text-xs font-medium text-neutral-600",
            "hover:bg-neutral-100",
            "rounded-lg transition-colors",
          ])}
        >
          <span>Load 5 more</span>
          <ChevronDownIcon className={cn(["h-3 w-3"])} />
        </button>
      )}
    </div>
  );
}
