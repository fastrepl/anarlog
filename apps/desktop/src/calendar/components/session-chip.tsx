import { format } from "date-fns";
import { useCallback } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import { toTz, useTimelineSession, useTimezone } from "~/calendar/hooks";
import { useTabs } from "~/store/zustand/tabs";

export function SessionChip({ sessionId }: { sessionId: string }) {
  const tz = useTimezone();
  const session = useTimelineSession(sessionId);

  if (!session || !session.title) {
    return null;
  }

  const createdAt = session.createdAt
    ? format(toTz(session.createdAt, tz), "h:mm a")
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
          <div className="w-[4px] shrink-0 self-stretch rounded-full border border-neutral-300 bg-transparent" />
          <span className="truncate">{session.title}</span>
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
  const session = useTimelineSession(sessionId);
  const openNew = useTabs((state) => state.openNew);
  const tz = useTimezone();

  const handleOpen = useCallback(() => {
    openNew({ type: "sessions", id: sessionId });
  }, [openNew, sessionId]);

  if (!session) {
    return null;
  }

  const createdAt = session.createdAt
    ? format(toTz(session.createdAt, tz), "MMM d, yyyy h:mm a")
    : null;

  return (
    <div className="flex flex-col gap-3 p-4">
      <div className="text-base font-medium text-neutral-900">
        {session.title}
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
