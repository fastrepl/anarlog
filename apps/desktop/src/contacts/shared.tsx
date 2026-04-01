import { stringHash } from "facehash";
import { ArrowDownUp, Plus, Search, X } from "lucide-react";
import type { KeyboardEvent, RefObject } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";

const COLOR_PALETTES = [
  "bg-amber-50",
  "bg-rose-50",
  "bg-violet-50",
  "bg-blue-50",
  "bg-teal-50",
  "bg-green-50",
  "bg-cyan-50",
  "bg-fuchsia-50",
  "bg-indigo-50",
  "bg-yellow-50",
];

export function getContactBgClass(name: string) {
  const hash = stringHash(name);
  return COLOR_PALETTES[hash % COLOR_PALETTES.length];
}

export type SortOption =
  | "alphabetical"
  | "reverse-alphabetical"
  | "oldest"
  | "newest";

export function SortDropdown({
  sortOption,
  setSortOption,
}: {
  sortOption: SortOption;
  setSortOption: (option: SortOption) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button size="icon" variant="ghost" aria-label="Sort options">
          <ArrowDownUp size={16} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end">
        <AppFloatingPanel className="overflow-hidden p-1">
          <DropdownMenuRadioGroup
            value={sortOption}
            onValueChange={(value) => setSortOption(value as SortOption)}
          >
            <DropdownMenuRadioItem
              value="alphabetical"
              className="cursor-pointer text-xs"
            >
              A-Z
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem
              value="reverse-alphabetical"
              className="cursor-pointer text-xs"
            >
              Z-A
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem
              value="oldest"
              className="cursor-pointer text-xs"
            >
              Oldest
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem
              value="newest"
              className="cursor-pointer text-xs"
            >
              Newest
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function ColumnHeader({
  title,
  sortOption,
  setSortOption,
  onAdd,
  searchValue,
  onSearchChange,
  searchInputRef,
}: {
  title: string;
  sortOption?: SortOption;
  setSortOption?: (option: SortOption) => void;
  onAdd: () => void;
  searchValue?: string;
  onSearchChange?: (value: string) => void;
  searchInputRef?: RefObject<HTMLInputElement | null>;
}) {
  const handleSearchKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      onSearchChange?.("");
    }
  };

  return (
    <div className="@container">
      <div className="flex h-12 min-w-0 items-center justify-between py-2 pr-1 pl-3">
        <h3 className="font-serif text-sm font-medium select-none">{title}</h3>
        <div className="flex shrink-0 items-center">
          {sortOption && setSortOption && (
            <div className="hidden @[220px]:block">
              <SortDropdown
                sortOption={sortOption}
                setSortOption={setSortOption}
              />
            </div>
          )}
          <Button onClick={onAdd} size="icon" variant="ghost" title="Add">
            <Plus size={16} />
          </Button>
        </div>
      </div>
      {onSearchChange && (
        <div className="px-2 pb-2">
          <div className="flex h-8 items-center gap-2 rounded-lg border border-neutral-200 bg-neutral-200/50 px-3 transition-colors focus-within:bg-neutral-200">
            <Search className="h-4 w-4 shrink-0 text-neutral-400" />
            <input
              ref={searchInputRef}
              type="text"
              value={searchValue || ""}
              onChange={(e) => onSearchChange(e.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder="Search contacts..."
              className="min-w-0 flex-1 bg-transparent text-sm placeholder:text-sm placeholder:text-neutral-400 focus:outline-hidden"
            />
            {searchValue && (
              <button
                onClick={() => onSearchChange("")}
                className="h-4 w-4 shrink-0 text-neutral-400 transition-colors hover:text-neutral-600"
                aria-label="Clear search"
              >
                <X className="h-4 w-4" />
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
