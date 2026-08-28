import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowsDownUp, MagnifyingGlass, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

import type { HumanSessionRecord } from "./queries";

export function RelatedNotesSection({
  sessions,
  onSessionClick,
}: {
  sessions: HumanSessionRecord[];
  onSessionClick: (id: string) => void;
}) {
  const { t } = useLingui();
  const [search, setSearch] = useState("");
  const [sortOrder, setSortOrder] = useState<"newest" | "oldest">("newest");
  const visibleSessions = sortAndFilterRelatedNotes(
    sessions,
    search,
    sortOrder,
  );

  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.header)}>
        <h3 {...stylex.props(styles.heading)}>
          <Trans>Related Notes</Trans>
        </h3>
        {sessions.length > 1 && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                size="icon"
                variant="ghost"
                sx={styles.sortButton}
                aria-label={t`Sort options`}
              >
                <ArrowsDownUp size={15} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent variant="app" align="start">
              <AppFloatingPanel sx={styles.menuPanel}>
                <DropdownMenuRadioGroup
                  value={sortOrder}
                  onValueChange={(value) =>
                    setSortOrder(value as "newest" | "oldest")
                  }
                >
                  <DropdownMenuRadioItem value="newest">
                    <Trans>Newest</Trans>
                  </DropdownMenuRadioItem>
                  <DropdownMenuRadioItem value="oldest">
                    <Trans>Oldest</Trans>
                  </DropdownMenuRadioItem>
                </DropdownMenuRadioGroup>
              </AppFloatingPanel>
            </DropdownMenuContent>
          </DropdownMenu>
        )}

        <div {...stylex.props(styles.searchField)}>
          <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
          <input
            type="text"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") setSearch("");
            }}
            placeholder={t`Search...`}
            {...stylex.props(styles.searchInput)}
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch("")}
              {...stylex.props(styles.clearButton)}
              aria-label={t`Clear search`}
            >
              <X {...stylex.props(styles.smallIcon)} />
            </button>
          )}
        </div>
      </div>

      {visibleSessions.length > 0 ? (
        <ul>
          {visibleSessions.map((session) => (
            <li key={session.id}>
              <button
                type="button"
                onClick={() => onSessionClick(session.id)}
                {...stylex.props(styles.noteButton)}
              >
                <span {...stylex.props(styles.bullet)} />
                <span {...stylex.props(styles.noteTitle)}>
                  {session.title || t`Untitled Note`}
                </span>
                {session.createdAt && (
                  <time {...stylex.props(styles.noteDate)}>
                    {new Date(session.createdAt).toLocaleDateString()}
                  </time>
                )}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p {...stylex.props(styles.empty)}>
          {sessions.length > 0 ? (
            <Trans>No results found.</Trans>
          ) : (
            <Trans>No related notes found</Trans>
          )}
        </p>
      )}
    </div>
  );
}

export function sortAndFilterRelatedNotes(
  sessions: HumanSessionRecord[],
  search: string,
  sortOrder: "newest" | "oldest",
): HumanSessionRecord[] {
  const normalizedSearch = search.trim().toLocaleLowerCase();
  const direction = sortOrder === "newest" ? -1 : 1;

  return sessions
    .filter(
      (session) =>
        !normalizedSearch ||
        session.title.toLocaleLowerCase().includes(normalizedSearch),
    )
    .sort((left, right) => {
      const dateDifference =
        getSessionTimestamp(left.createdAt) -
        getSessionTimestamp(right.createdAt);
      return dateDifference !== 0
        ? dateDifference * direction
        : left.id.localeCompare(right.id) * direction;
    });
}

function getSessionTimestamp(value: string): number {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? 0 : timestamp;
}

const styles = stylex.create({
  bullet: {
    backgroundColor: colors.mutedForeground,
    borderRadius: radii.full,
    flexShrink: 0,
    height: "0.375rem",
    width: "0.375rem",
  },
  clearButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    flexShrink: 0,
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.5rem",
  },
  header: {
    alignItems: "center",
    display: "flex",
    gap: "0.25rem",
    marginBottom: "0.75rem",
  },
  heading: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  noteButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.md,
    display: "flex",
    gap: "0.75rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.5rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  noteDate: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  noteTitle: {
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  root: {
    padding: "1.5rem",
  },
  searchField: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in srgb, ${colors.muted} 50%, transparent)`,
      ":focus-within": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    height: "2rem",
    marginLeft: "auto",
    maxWidth: "48%",
    paddingInline: "0.625rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "13rem",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
  searchInput: {
    "::placeholder": {
      color: colors.mutedForeground,
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
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  sortButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    height: "1.75rem",
    width: "1.75rem",
  },
});
