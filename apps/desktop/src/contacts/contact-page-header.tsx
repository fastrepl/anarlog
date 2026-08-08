import { Trans, useLingui } from "@lingui/react/macro";
import { DotsThree, PushPin, Trash } from "@phosphor-icons/react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

export function ContactPageHeader({
  pinned,
  onTogglePin,
  onDelete,
}: {
  pinned: boolean;
  onTogglePin: () => void;
  onDelete: () => void;
}) {
  const { t } = useLingui();

  return (
    <div
      data-tauri-drag-region
      className="flex h-12 shrink-0 items-start justify-end py-0 pt-[9px] pr-1 pl-2"
    >
      <div
        data-tauri-drag-region="false"
        className="flex shrink-0 items-center"
      >
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              size="icon"
              variant="ghost"
              data-tauri-drag-region="false"
              className="text-muted-foreground hover:bg-accent hover:text-foreground rounded-full"
              aria-label={t`Contact options`}
            >
              <DotsThree size={16} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent variant="app" align="end" className="w-48">
            <AppFloatingPanel className="overflow-hidden p-1">
              <DropdownMenuItem
                onClick={onTogglePin}
                className="cursor-pointer"
              >
                <PushPin weight={pinned ? "fill" : "regular"} />
                <span>
                  {pinned ? <Trans>Unpin</Trans> : <Trans>Pin</Trans>}
                </span>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={onDelete}
                className={cn([
                  "cursor-pointer text-red-600 dark:text-red-400",
                  "hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-950/50 dark:hover:text-red-300",
                ])}
              >
                <Trash />
                <span>
                  <Trans>Delete</Trans>
                </span>
              </DropdownMenuItem>
            </AppFloatingPanel>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
