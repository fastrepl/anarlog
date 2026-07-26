import { format } from "date-fns";
import { useCallback, useMemo } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import { toTz, useTimezone } from "~/calendar/hooks";
import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { getSessionEvent } from "~/session/utils";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import type { TimelineSessionRow } from "~/sidebar/timeline/utils";
import { useTabs } from "~/store/zustand/tabs";

export function SessionChip({
  sessionId,
  session,
}: {
  sessionId: string;
  session: TimelineSessionRow | undefined;
}) {
  const tz = useTimezone();
  const deleteSession = useDeleteSession();
  const title = session?.title ?? undefined;
  const eventJson = session?.event_json;
  const createdAt = session?.created_at
    ? format(toTz(session.created_at, tz), "h:mm a")
    : null;

  const handleShowInFinder = useCallback(async () => {
    const result = await fsSyncCommands.sessionDir(sessionId);
    if (result.status === "ok") {
      await openerCommands.openPath(result.data, null);
    }
  }, [sessionId]);

  const handleDelete = useCallback(() => {
    const sessionEvent = getSessionEvent({ event_json: eventJson });
    deleteSession(sessionId, {
      trackingId: sessionEvent?.tracking_id,
      title,
    });
  }, [deleteSession, sessionId, eventJson, title]);

  const contextMenu = useMemo<MenuItemDef[]>(
    () => [
      {
        id: "show",
        text: "Show in Finder",
        action: handleShowInFinder,
      },
      { separator: true },
      {
        id: "delete",
        text: "Delete Note",
        action: handleDelete,
      },
    ],
    [handleShowInFinder, handleDelete],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  if (!session || !title) {
    return null;
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          className={cn([
            "flex w-full items-center gap-1 rounded pl-0.5 text-left text-xs leading-tight",
            "cursor-pointer select-none hover:opacity-80",
          ])}
          onContextMenu={showContextMenu}
        >
          <div className="border-border w-[4px] shrink-0 self-stretch rounded-full border bg-transparent" />
          <span className="truncate">{title}</span>
          {createdAt && (
            <span className="text-muted-foreground ml-auto shrink-0 font-mono">
              {createdAt}
            </span>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="start"
        className="w-[280px]"
        onClick={(e) => e.stopPropagation()}
      >
        <AppFloatingPanel>
          <SessionPopoverContent sessionId={sessionId} session={session} />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function SessionPopoverContent({
  sessionId,
  session,
}: {
  sessionId: string;
  session: TimelineSessionRow;
}) {
  const openCurrent = useTabs((state) => state.openCurrent);
  const tz = useTimezone();

  const handleOpen = useCallback(() => {
    openCurrent({ type: "sessions", id: sessionId });
  }, [openCurrent, sessionId]);

  const createdAt = session.created_at
    ? format(toTz(session.created_at, tz), "MMM d, yyyy h:mm a")
    : null;

  return (
    <div className="flex flex-col gap-3 p-4">
      <div className="text-foreground text-base font-medium">
        {session.title}
      </div>
      <div className="bg-accent h-px" />
      {createdAt && (
        <div className="text-muted-foreground text-sm">{createdAt}</div>
      )}
      <Button
        size="sm"
        className="bg-primary text-primary-foreground hover:bg-primary/90 min-h-8 w-full"
        onClick={handleOpen}
      >
        Open note
      </Button>
    </div>
  );
}
