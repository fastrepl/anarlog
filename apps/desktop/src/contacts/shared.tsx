import { Facehash, stringHash } from "facehash";
import { ArrowDownUp, Plus, Search, X } from "lucide-react";
import type { ComponentProps, KeyboardEvent, RefObject } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { cn } from "@hypr/utils";

import { CustomSidebarHeader } from "~/sidebar/custom-sidebar-header";

const COLOR_PALETTES = [
  "bg-amber-50 dark:bg-amber-950",
  "bg-rose-50 dark:bg-rose-950",
  "bg-violet-50 dark:bg-violet-950",
  "bg-blue-50 dark:bg-blue-950",
  "bg-teal-50 dark:bg-teal-950",
  "bg-green-50 dark:bg-green-950",
  "bg-cyan-50 dark:bg-cyan-950",
  "bg-fuchsia-50 dark:bg-fuchsia-950",
  "bg-indigo-50 dark:bg-indigo-950",
  "bg-yellow-50 dark:bg-yellow-950",
] as const;

export const CONTACT_FACEHASH_CLASS = "text-foreground";

export function getContactBgClass(name: string) {
  const hash = stringHash(name);
  return COLOR_PALETTES[hash % COLOR_PALETTES.length];
}

export function ContactFacehash({
  name,
  className,
  colorClasses,
  ...props
}: ComponentProps<typeof Facehash>) {
  const bgClass = colorClasses?.[0] ?? getContactBgClass(name);

  return (
    <Facehash
      name={name}
      className={cn([CONTACT_FACEHASH_CLASS, className])}
      colorClasses={colorClasses ?? [bgClass]}
      {...props}
    />
  );
}

export type SortOption =
  | "alphabetical"
  | "reverse-alphabetical"
  | "oldest"
  | "newest";

function SortDropdown({
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
      <CustomSidebarHeader title={title}>
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
      </CustomSidebarHeader>
      {onSearchChange && (
        <div className="pb-2">
          <div className="border-border bg-muted focus-within:bg-accent flex h-8 w-full items-center gap-2 rounded-lg border px-3 transition-colors">
            <Search className="text-muted-foreground h-4 w-4 shrink-0" />
            <input
              ref={searchInputRef}
              type="text"
              value={searchValue || ""}
              onChange={(e) => onSearchChange(e.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder="Search contacts..."
              className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm placeholder:text-sm focus:outline-hidden"
            />
            {searchValue && (
              <button
                onClick={() => onSearchChange("")}
                className="text-muted-foreground hover:text-foreground h-4 w-4 shrink-0 transition-colors"
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
