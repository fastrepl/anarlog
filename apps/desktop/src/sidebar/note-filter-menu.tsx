import { Trans, useLingui } from "@lingui/react/macro";
import { FolderSimple, FunnelSimple } from "@phosphor-icons/react";

import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import {
  decodeNotesView,
  encodeNotesView,
  type SidebarNoteFilter,
} from "./note-filter";

import { useFolderPaths } from "~/session/queries";

export function SidebarNoteFilterMenu({
  folderFilter = null,
  value,
  onValueChange,
}: {
  folderFilter?: string | null;
  value: SidebarNoteFilter;
  onValueChange: (
    filter: SidebarNoteFilter,
    folderFilter?: string | null,
  ) => void;
}) {
  const { t } = useLingui();
  const folders = useFolderPaths();
  const encodedValue = encodeNotesView(value, folderFilter);
  const showFolderSection = folders.length > 0;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t`Filter notes`}
          title={t`Filter notes`}
          data-tauri-drag-region="false"
          className={cn([
            "pointer-events-auto relative flex size-7 items-center justify-center rounded-full",
            "text-muted-foreground hover:bg-accent hover:text-foreground transition-colors",
            "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
            encodedValue !== "mine" && "bg-accent text-foreground",
          ])}
        >
          <FunnelSimple
            size={15}
            weight={encodedValue === "mine" ? "regular" : "fill"}
          />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="start" className="w-52">
        <AppFloatingPanel className="overflow-hidden p-1">
          <DropdownMenuRadioGroup
            value={encodedValue}
            onValueChange={(nextValue) => {
              const next = decodeNotesView(nextValue);
              onValueChange(next.noteFilter, next.folderFilter);
            }}
          >
            <DropdownMenuRadioItem value="mine">
              <Trans>My notes</Trans>
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="shared">
              <Trans>Shared</Trans>
            </DropdownMenuRadioItem>
            {showFolderSection ? (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuRadioItem value="folder:">
                  <Trans>No folder</Trans>
                </DropdownMenuRadioItem>
                {folders.map((path) => (
                  <DropdownMenuRadioItem key={path} value={`folder:${path}`}>
                    <FolderSimple
                      className="size-4 shrink-0 opacity-70"
                      aria-hidden="true"
                    />
                    <span className="min-w-0 flex-1 truncate">{path}</span>
                  </DropdownMenuRadioItem>
                ))}
              </>
            ) : null}
          </DropdownMenuRadioGroup>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
