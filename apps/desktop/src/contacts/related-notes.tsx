import { Trans, useLingui } from "@lingui/react/macro";
import { ArrowsDownUp, MagnifyingGlass, X } from "@phosphor-icons/react";
import { useState } from "react";

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
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";

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
  const searchRef = useSquircleRef<HTMLDivElement>();
  const [sortOrder, setSortOrder] = useState<"newest" | "oldest">("newest");
  const visibleSessions = sortAndFilterRelatedNotes(
    sessions,
    search,
    sortOrder,
  );

  return (
    <div className="p-6">
      <div className="mb-3 flex items-center gap-1">
        <h3 className="text-muted-foreground text-sm font-medium">
          <Trans>Related Notes</Trans>
        </h3>
        {sessions.length > 1 && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                size="icon"
                variant="ghost"
                className="text-muted-foreground hover:text-foreground size-7"
                aria-label={t`Sort options`}
              >
                <ArrowsDownUp size={15} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent variant="app" align="start">
              <AppFloatingPanel className={appFloatingMenuPanelClassName}>
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

        <div
          ref={searchRef}
          className="border-border bg-muted/50 focus-within:bg-accent ml-auto flex h-8 w-52 max-w-[48%] items-center gap-2 rounded-lg border px-2.5 transition-colors"
        >
          <MagnifyingGlass className="text-muted-foreground size-3.5 shrink-0" />
          <input
            type="text"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") setSearch("");
            }}
            placeholder={t`Search...`}
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm focus:outline-hidden"
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch("")}
              className="text-muted-foreground hover:text-foreground shrink-0 transition-colors"
              aria-label={t`Clear search`}
            >
              <X className="size-3.5" />
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
                className="hover:bg-accent flex w-full items-center gap-3 rounded-md px-2 py-2 text-left transition-colors"
              >
                <span className="bg-muted-foreground size-1.5 shrink-0 rounded-full" />
                <span className="min-w-0 flex-1 truncate text-sm">
                  {session.title || t`Untitled Note`}
                </span>
                {session.createdAt && (
                  <time className="text-muted-foreground shrink-0 text-xs">
                    {new Date(session.createdAt).toLocaleDateString()}
                  </time>
                )}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-muted-foreground px-2 py-2 text-sm">
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
