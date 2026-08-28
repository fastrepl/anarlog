import { Trans, useLingui } from "@lingui/react/macro";
import { FunnelSimple } from "@phosphor-icons/react";

import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import type { SidebarNoteFilter } from "./note-filter";

export function SidebarNoteFilterMenu({
  value,
  onValueChange,
}: {
  value: SidebarNoteFilter;
  onValueChange: (value: SidebarNoteFilter) => void;
}) {
  const { t } = useLingui();

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
            value !== "mine" && "bg-accent text-foreground",
          ])}
        >
          <FunnelSimple
            size={15}
            weight={value === "mine" ? "regular" : "fill"}
          />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent variant="app" align="start" className="w-52">
        <AppFloatingPanel className="overflow-hidden p-1">
          <DropdownMenuRadioGroup
            value={value}
            onValueChange={(nextValue) =>
              onValueChange(nextValue as SidebarNoteFilter)
            }
          >
            <DropdownMenuRadioItem value="mine">
              <Trans>My notes</Trans>
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="shared">
              <Trans>Shared</Trans>
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
