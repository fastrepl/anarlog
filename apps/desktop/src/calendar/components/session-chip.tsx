import { format } from "date-fns";
import { useCallback } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import { toTz, useTimezone } from "~/calendar/hooks";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

export function SessionChip({ sessionId }: { sessionId: string }) {
  const tz = useTimezone();
  const session = main.UI.useResultRow(
    main.QUERIES.timelineSessions,
    sessionId,
    main.STORE_ID,
  );

  if (!session || !session.title) {
    return null;
  }

  const createdAt = session.created_at
    ? format(toTz(session.created_at as string, tz), "h:mm a")
    : null;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          className={cn([
            "flex w-full items-center gap-1 rounded pl-0.5 text-left text-xs leading-tight",
            "cursor-pointer hover:opacity-80",
          ])}
        >
          <div className="w-[2.5px] shrink-0 self-stretch rounded-full bg-blue-500" />
          <span className="truncate">{session.title as string}</span>
          {createdAt && (
            <span className="ml-auto shrink-0 font-mono text-neutral-400">
              {createdAt}
            </span>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-[280px] rounded-lg p-0 shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <SessionPopoverContent sessionId={sessionId} />
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
