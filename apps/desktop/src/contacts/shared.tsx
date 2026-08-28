import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowsDownUp, MagnifyingGlass, Plus, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { KeyboardEvent, RefObject } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Avatar } from "@anlg/ui/components/avatar";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
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
  sx,
}: {
  name: string;
  size?: number;
  sx?: stylex.StyleXStyles;
}) {
  return <Avatar seed={name} label={name} size={size} sx={sx} />;
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
          sx={styles.iconButton}
          aria-label={t`Sort options`}
        >
          <ArrowsDownUp size={16} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="end">
        <AppFloatingPanel sx={styles.menuPanel}>
          <DropdownMenuRadioGroup
            value={sortOption}
            onValueChange={(value) => setSortOption(value as SortOption)}
          >
            <DropdownMenuRadioItem value="alphabetical" sx={styles.menuItem}>
              A-Z
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem
              value="reverse-alphabetical"
              sx={styles.menuItem}
            >
              Z-A
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="oldest" sx={styles.menuItem}>
              <Trans>Oldest</Trans>
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="newest" sx={styles.menuItem}>
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
    <div {...stylex.props(styles.container)}>
      <CustomSidebarHeader>
        <div {...stylex.props(styles.headerActions)}>
          {sortOption && setSortOption && (
            <div {...stylex.props(styles.sortControl)}>
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
            sx={styles.iconButton}
            title={t`Add`}
          >
            <Plus size={16} />
          </Button>
        </div>
      </CustomSidebarHeader>
      {onSearchChange && (
        <div {...stylex.props(styles.searchSection)}>
          <div {...stylex.props(styles.searchField)}>
            <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
            <input
              ref={searchInputRef}
              type="text"
              value={searchValue || ""}
              onChange={(e) => onSearchChange(e.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder={t`Search contacts...`}
              {...stylex.props(styles.searchInput)}
            />
            {searchValue && (
              <button
                onClick={() => onSearchChange("")}
                {...stylex.props(styles.clearButton)}
                aria-label={t`Clear search`}
              >
                <X {...stylex.props(styles.clearIcon)} />
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

const styles = stylex.create({
  clearButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    flexShrink: 0,
    height: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  clearIcon: {
    height: "1rem",
    width: "1rem",
  },
  container: {
    containerType: "inline-size",
  },
  headerActions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
  },
  iconButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  menuItem: {
    cursor: "pointer",
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  searchField: {
    alignItems: "center",
    backgroundColor: {
      default: colors.muted,
      ":focus-within": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    height: "2rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  searchInput: {
    "::placeholder": {
      color: colors.mutedForeground,
      fontSize: "0.875rem",
    },
    backgroundColor: "transparent",
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    outline: {
      default: null,
      ":focus": "none",
    },
  },
  searchSection: {
    paddingBottom: "0.5rem",
  },
  sortControl: {
    display: {
      default: "none",
      "@container (min-width: 220px)": "block",
    },
  },
});
