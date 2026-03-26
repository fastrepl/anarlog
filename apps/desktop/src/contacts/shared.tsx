import { stringHash } from "facehash";
import { ArrowDownUp, Plus, Search, X } from "lucide-react";
import React from "react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
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
      <DropdownMenuContent align="end">
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
  showSearch: showSearchProp,
  onShowSearchChange,
}: {
  title: string;
  sortOption?: SortOption;
  setSortOption?: (option: SortOption) => void;
  onAdd: () => void;
  searchValue?: string;
  onSearchChange?: (value: string) => void;
  showSearch?: boolean;
  onShowSearchChange?: (show: boolean) => void;
}) {
  const [showSearchInternal, setShowSearchInternal] = useState(false);
  const showSearch = showSearchProp ?? showSearchInternal;
  const setShowSearch = onShowSearchChange ?? setShowSearchInternal;

  const handleSearchToggle = () => {
    if (showSearch) {
      onSearchChange?.("");
    }
    setShowSearch(!showSearch);
  };

  const handleSearchKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      onSearchChange?.("");
      setShowSearch(false);
      e.currentTarget.blur();
    }
  };

  return (
    <div className="@container border-b border-neutral-200">
      <div className="flex h-12 min-w-0 items-center justify-between py-2 pr-1 pl-3">
        <h3 className="text-sm font-medium">{title}</h3>
        <div className="flex shrink-0 items-center">
          {onSearchChange && (
            <Button
              onClick={handleSearchToggle}
              size="icon"
              variant="ghost"
              title="Search"
            >
              <Search size={16} />
            </Button>
          )}
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
      {showSearch && onSearchChange && (
        <div className="flex h-12 items-center gap-2 border-t border-neutral-200 bg-white px-3">
          <Search className="h-4 w-4 shrink-0 text-neutral-400" />
          <input
            type="text"
            value={searchValue || ""}
            onChange={(e) => onSearchChange(e.target.value)}
            onKeyDown={handleSearchKeyDown}
            placeholder="Search..."
            className="w-full bg-transparent text-sm placeholder:text-neutral-400 focus:outline-hidden"
            autoFocus
          />
          {searchValue && (
            <button
              onClick={() => onSearchChange("")}
              className="shrink-0 rounded-xs p-1 transition-colors hover:bg-neutral-100"
            >
              <X className="h-4 w-4 text-neutral-400" />
            </button>
          )}
        </div>
      )}
    </div>
  );
}
