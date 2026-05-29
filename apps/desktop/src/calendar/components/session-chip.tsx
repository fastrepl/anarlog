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
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

export function SessionChip({ sessionId }: { sessionId: string }) {
  const tz = useTimezone();
  const openNew = useTabs((state) => state.openNew);
  const deleteSession = useDeleteSession();
  const session = main.UI.useResultRow(
    main.QUERIES.timelineSessions,
    sessionId,
    main.STORE_ID,
  );
  const title = session?.title as string | undefined;
  const eventJson = session?.event_json as string | null | undefined;
  const createdAt = session?.created_at
    ? format(toTz(session.created_at as string, tz), "h:mm a")
    : null;

  const handleOpenNewTab = useCallback(() => {
    openNew({ type: "sessions", id: sessionId });
  }, [openNew, sessionId]);

  const handleShowInFinder = useCallback(async () => {
    const result = await fsSyncCommands.sessionDir(sessionId);
    if (result.status === "ok") {
      await openerCommands.openPath(result.data, null);
    }
  }, [sessionId]);

  const handleDelete = useCallback(() => {
    const sessionEvent = getSessionEvent({ event_json: eventJson });
    deleteSession(sessionId, sessionEvent?.tracking_id);
  }, [deleteSession, sessionId, eventJson]);

  const contextMenu = useMemo<MenuItemDef[]>(
    () => [
      {
        id: "open-new-tab",
        text: "Open in New Tab",
        action: handleOpenNewTab,
      },
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
    [handleOpenNewTab, handleShowInFinder, handleDelete],
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
          <div className="w-[4px] shrink-0 self-stretch rounded-full border border-neutral-300 bg-transparent" />
          <span className="truncate">{title}</span>
          {createdAt && (
            <span className="ml-auto shrink-0 font-mono text-neutral-400">
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
          <SessionPopoverContent sessionId={sessionId} />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function SessionPopoverContent({ sessionId }: { sessionId: string }) {
  const session = main.UI.useResultRow(
    main.QUERIES.timelineSessions,
    sessionId,
    main.STORE_ID,
  );
  const openNew = useTabs((state) => state.openNew);
  const tz = useTimezone();

  const handleOpen = useCallback(() => {
    openNew({ type: "sessions", id: sessionId });
  }, [openNew, sessionId]);

  if (!session) {
    return null;
  }

  const createdAt = session.created_at
    ? format(toTz(session.created_at as string, tz), "MMM d, yyyy h:mm a")
    : null;

  return (
    <div className="flex flex-col gap-3 p-4">
      <div className="text-base font-medium text-neutral-900">
        {session.title as string}
      </div>
      <div className="h-px bg-neutral-200" />
      {createdAt && <div className="text-sm text-neutral-700">{createdAt}</div>}
      <Button
        size="sm"
        className="min-h-8 w-full bg-stone-800 text-white hover:bg-stone-700"
        onClick={handleOpen}
      >
        Open note
      </Button>
    </div>
  );
}
