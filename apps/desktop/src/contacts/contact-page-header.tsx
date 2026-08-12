import { Trans, useLingui } from "@lingui/react/macro";
import { DotsThree, MinusCircle, PushPin, Trash } from "@phosphor-icons/react";
import { type ReactNode } from "react";

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
  title,
  compactIdentity,
  showCompactIdentity,
  pinned,
  onTogglePin,
  onDelete,
  onRemoveAvatar,
}: {
  title: string;
  compactIdentity: ReactNode;
  showCompactIdentity: boolean;
  pinned: boolean;
  onTogglePin: () => void;
  onDelete: () => void;
  onRemoveAvatar?: () => void;
}) {
  const { t } = useLingui();

  return (
    <div
      data-tauri-drag-region
      className="flex h-12 shrink-0 items-center justify-between gap-3 pr-1 pl-3"
    >
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {showCompactIdentity && compactIdentity}
        <h2 className="min-w-0 truncate text-sm font-semibold">{title}</h2>
      </div>
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
              {onRemoveAvatar && (
                <DropdownMenuItem
                  onClick={onRemoveAvatar}
                  className="cursor-pointer"
                >
                  <MinusCircle />
                  <span>
                    <Trans>Remove photo</Trans>
                  </span>
                </DropdownMenuItem>
              )}
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
