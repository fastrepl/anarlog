import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowsDownUp, MagnifyingGlass, Plus, X } from "@phosphor-icons/react";
import type { KeyboardEvent, RefObject } from "react";

import { Avatar } from "@anlg/ui/components/avatar";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

import { CustomSidebarHeader } from "~/sidebar/custom-sidebar-header";

export function ContactFacehash({
  name,
  size = 40,
  className,
}: {
  name: string;
  size?: number;
  className?: string;
}) {
  return <Avatar seed={name} label={name} size={size} className={className} />;
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
  const { t } = useLingui();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          size="icon"
          variant="ghost"
          className="text-muted-foreground hover:text-foreground"
          aria-label={t`Sort options`}
        >
          <ArrowsDownUp size={16} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end">
        <AppFloatingPanel className={appFloatingMenuPanelClassName}>
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
              <Trans>Oldest</Trans>
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem
              value="newest"
              className="cursor-pointer text-xs"
            >
              <Trans>Newest</Trans>
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function ColumnHeader({
  sortOption,
  setSortOption,
  onAdd,
  searchValue,
  onSearchChange,
  searchInputRef,
}: {
  sortOption?: SortOption;
  setSortOption?: (option: SortOption) => void;
  onAdd: () => void;
  searchValue?: string;
  onSearchChange?: (value: string) => void;
  searchInputRef?: RefObject<HTMLInputElement | null>;
}) {
  const { t } = useLingui();
  const handleSearchKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      onSearchChange?.("");
    }
  };

  return (
    <div className="@container">
      <CustomSidebarHeader>
        <div className="flex shrink-0 items-center">
          {sortOption && setSortOption && (
            <div className="hidden @[220px]:block">
              <SortDropdown
                sortOption={sortOption}
                setSortOption={setSortOption}
              />
            </div>
          )}
          <Button
            onClick={onAdd}
            size="icon"
            variant="ghost"
            className="text-muted-foreground hover:text-foreground"
            title={t`Add`}
          >
            <Plus size={16} />
          </Button>
        </div>
      </CustomSidebarHeader>
      {onSearchChange && (
        <div className="pb-2">
          <div className="border-border bg-muted focus-within:bg-accent flex h-8 w-full items-center gap-2 rounded-lg border px-3 transition-colors">
            <MagnifyingGlass className="text-muted-foreground h-4 w-4 shrink-0" />
            <input
              ref={searchInputRef}
              type="text"
              value={searchValue || ""}
              onChange={(e) => onSearchChange(e.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder={t`Search contacts...`}
              className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm placeholder:text-sm focus:outline-hidden"
            />
            {searchValue && (
              <button
                onClick={() => onSearchChange("")}
                className="text-muted-foreground hover:text-foreground h-4 w-4 shrink-0 transition-colors"
                aria-label={t`Clear search`}
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
