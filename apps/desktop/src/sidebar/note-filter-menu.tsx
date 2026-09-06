import { Trans, useLingui } from "@lingui/react/macro";

import {
  CalendarBlank,
  Check,
  FolderSimple,
  FunnelSimple,
  SortAscending,
  SortDescending,
} from "@anlg/ui/components/icons";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import { useSidebarNotes } from "./note-filter";

export function SidebarNoteFilterMenu() {
  const { t } = useLingui();
  const groupBy = useSidebarNotes((state) => state.groupBy);
  const sortOrder = useSidebarNotes((state) => state.sortOrder);
  const setGroupBy = useSidebarNotes((state) => state.setGroupBy);
  const setSortOrder = useSidebarNotes((state) => state.setSortOrder);
  const isDefaultView = groupBy === "date" && sortOrder === "newest";
  const groupingLabel = groupBy === "folder" ? t`Folder` : t`Date`;
  const orderingLabel = sortOrder === "oldest" ? t`Oldest` : t`Newest`;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t`Sort notes`}
          title={t`Sort notes`}
          data-tauri-drag-region="false"
          className={cn([
            "pointer-events-auto relative flex size-7 items-center justify-center rounded-full",
            "text-muted-foreground hover:bg-accent hover:text-foreground transition-colors",
            "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
            !isDefaultView && "bg-accent text-foreground",
          ])}
        >
          <FunnelSimple size={15} weight={isDefaultView ? "regular" : "bold"} />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="start" className="w-56">
        <AppFloatingPanel className={appFloatingMenuPanelClassName}>
          <DropdownMenuSub>
            <DropdownMenuSubTrigger
              aria-label={t`Grouping, ${groupingLabel}`}
              className="cursor-pointer"
            >
              <span className="flex-1">
                <Trans>Grouping</Trans>
              </span>
              <span className="text-muted-foreground">{groupingLabel}</span>
            </DropdownMenuSubTrigger>
            <DropdownMenuPortal>
              <DropdownMenuSubContent variant="app" className="w-44">
                <AppFloatingPanel className={appFloatingMenuPanelClassName}>
                  <DropdownMenuItem
                    onSelect={() => setGroupBy("date")}
                    className="cursor-pointer"
                  >
                    <CalendarBlank
                      className="size-4 shrink-0 opacity-70"
                      aria-hidden="true"
                    />
                    <span className="flex-1">
                      <Trans>Date</Trans>
                    </span>
                    {groupBy === "date" ? (
                      <Check className="size-4" aria-hidden="true" />
                    ) : null}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onSelect={() => setGroupBy("folder")}
                    className="cursor-pointer"
                  >
                    <FolderSimple
                      className="size-4 shrink-0 opacity-70"
                      aria-hidden="true"
                    />
                    <span className="flex-1">
                      <Trans>Folder</Trans>
                    </span>
                    {groupBy === "folder" ? (
                      <Check className="size-4" aria-hidden="true" />
                    ) : null}
                  </DropdownMenuItem>
                </AppFloatingPanel>
              </DropdownMenuSubContent>
            </DropdownMenuPortal>
          </DropdownMenuSub>
          <DropdownMenuSub>
            <DropdownMenuSubTrigger
              aria-label={t`Ordering, ${orderingLabel}`}
              className="cursor-pointer"
            >
              <span className="flex-1">
                <Trans>Ordering</Trans>
              </span>
              <span className="text-muted-foreground">{orderingLabel}</span>
            </DropdownMenuSubTrigger>
            <DropdownMenuPortal>
              <DropdownMenuSubContent variant="app" className="w-44">
                <AppFloatingPanel className={appFloatingMenuPanelClassName}>
                  <DropdownMenuItem
                    onSelect={() => setSortOrder("newest")}
                    className="cursor-pointer"
                  >
                    <SortDescending
                      className="size-4 shrink-0 opacity-70"
                      aria-hidden="true"
                    />
                    <span className="flex-1">
                      <Trans>Newest</Trans>
                    </span>
                    {sortOrder === "newest" ? (
                      <Check className="size-4" aria-hidden="true" />
                    ) : null}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onSelect={() => setSortOrder("oldest")}
                    className="cursor-pointer"
                  >
                    <SortAscending
                      className="size-4 shrink-0 opacity-70"
                      aria-hidden="true"
                    />
                    <span className="flex-1">
                      <Trans>Oldest</Trans>
                    </span>
                    {sortOrder === "oldest" ? (
                      <Check className="size-4" aria-hidden="true" />
                    ) : null}
                  </DropdownMenuItem>
                </AppFloatingPanel>
              </DropdownMenuSubContent>
            </DropdownMenuPortal>
          </DropdownMenuSub>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
