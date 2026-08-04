import { Trans, useLingui } from "@lingui/react/macro";
import { FunnelSimple } from "@phosphor-icons/react";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import type { SidebarNoteFilter } from "./note-filter";

import { useAuth } from "~/auth";
import { useAvailableShareWorkspaces } from "~/session-sharing/source";

export function SidebarNoteFilterMenu({
  value,
  onValueChange,
}: {
  value: SidebarNoteFilter;
  onValueChange: (value: SidebarNoteFilter) => void;
}) {
  const { t } = useLingui();
  const { session } = useAuth();
  const workspaces = useAvailableShareWorkspaces(session?.user.id);

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
            value !== "all" && "bg-accent text-foreground",
          ])}
        >
          <FunnelSimple
            size={15}
            weight={value === "all" ? "regular" : "fill"}
          />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-52">
        <DropdownMenuRadioGroup
          value={value}
          onValueChange={(nextValue) =>
            onValueChange(nextValue as SidebarNoteFilter)
          }
        >
          <DropdownMenuRadioItem value="all">
            <Trans>All notes</Trans>
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="mine">
            <Trans>My notes</Trans>
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="shared-by-me">
            <Trans>Shared by me</Trans>
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="shared-with-me">
            <Trans>Shared with me</Trans>
          </DropdownMenuRadioItem>
          {workspaces.length > 0 ? <DropdownMenuSeparator /> : null}
          {workspaces.map((workspace) => (
            <DropdownMenuRadioItem
              key={workspace.id}
              value={`workspace:${workspace.id}`}
            >
              <span className="truncate">{workspace.name}</span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
