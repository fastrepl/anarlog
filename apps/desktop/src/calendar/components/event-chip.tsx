import { t } from "@lingui/core/macro";
import { useMutation } from "@tanstack/react-query";
import { format } from "date-fns";
import { useCallback, useMemo, useRef, useState } from "react";

import {
  CaretDown,
  CaretLeft,
  CircleNotch,
  Folder,
  Repeat,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { cn } from "@anlg/utils";

import { toTz, useTimezone } from "~/calendar/hooks";
import { useIgnoredEvents } from "~/calendar/ignored-events";
import {
  clearSeriesFolderRule,
  setSeriesFolderRuleAndAlign,
  useSeriesFolderRules,
} from "~/calendar/series-folders";
import { useLiveQuery } from "~/db";
import { FolderPickerContent } from "~/session/components/folder-picker";
import { EventDisplay } from "~/session/components/outer-header/metadata";
import { folderDisplayName } from "~/session/folders";
import { getOrCreateSessionForEventId } from "~/session/queries";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { useTimeFormat } from "~/shared/hooks/useTimeFormat";
import type { TimelineEventRow } from "~/sidebar/timeline/utils";
import { useTabs } from "~/store/zustand/tabs";

export function EventChip({
  eventId,
  event,
}: {
  eventId: string;
  event: TimelineEventRow | undefined;
}) {
  const tz = useTimezone();
  const timeFormat = useTimeFormat();
  const { ignoreEvent, ignoreSeries } = useIgnoredEvents();
  const title = event?.title || t`Busy`;
  const trackingId = event?.tracking_id_event ?? undefined;
  const recurrenceSeriesId = event?.recurrence_series_id ?? undefined;
  const isAllDay = !!event?.is_all_day;
  const color = event?.calendar_color || "#888";

  const startedAt = event?.started_at
    ? format(toTz(event.started_at, tz), timeFormat)
    : null;

  const handleIgnore = useCallback(() => {
    if (!trackingId) {
      return;
    }

    ignoreEvent(trackingId);
  }, [trackingId, ignoreEvent]);

  const handleIgnoreSeries = useCallback(() => {
    if (!recurrenceSeriesId) {
      return;
    }

    ignoreSeries(recurrenceSeriesId);
  }, [recurrenceSeriesId, ignoreSeries]);

  const contextMenu = useMemo<MenuItemDef[]>(() => {
    const menu: MenuItemDef[] = [
      {
        id: "ignore",
        text: recurrenceSeriesId ? "Delete This Event" : "Delete Event",
        action: handleIgnore,
      },
    ];

    if (recurrenceSeriesId) {
      menu.push({
        id: "ignore-series",
        text: "Delete All Recurring Events",
        action: handleIgnoreSeries,
      });
    }

    return menu;
  }, [recurrenceSeriesId, handleIgnore, handleIgnoreSeries]);
  const showContextMenu = useNativeContextMenu(contextMenu);

  if (!event) {
    return null;
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        {isAllDay ? (
          <button
            className={cn([
              "text-primary-foreground w-full truncate rounded px-1.5 py-0.5 text-left text-xs leading-tight",
              "cursor-pointer select-none hover:opacity-80",
            ])}
            style={{ backgroundColor: color }}
            onContextMenu={showContextMenu}
          >
            {title}
          </button>
        ) : (
          <button
            className={cn([
              "flex w-full items-center gap-1 rounded pl-0.5 text-left text-xs leading-tight",
              "cursor-pointer select-none hover:opacity-80",
            ])}
            onContextMenu={showContextMenu}
          >
            <div
              className="w-[2.5px] shrink-0 self-stretch rounded-full"
              style={{ backgroundColor: color }}
            />
            <span className="truncate">{title}</span>
            {startedAt && (
              <span className="text-muted-foreground ml-auto shrink-0 font-mono">
                {startedAt}
              </span>
            )}
          </button>
        )}
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        align="start"
        collisionPadding={16}
        className="flex max-h-[min(80vh,36rem)] w-[min(320px,calc(100vw-32px))] min-w-0 flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <AppFloatingPanel className="min-h-0 overflow-x-hidden overflow-y-auto">
          <EventPopoverContent eventId={eventId} event={event} />
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

type LinkedSessionSqlRow = {
  id: string;
  folder_path: string;
};

function EventPopoverContent({
  eventId,
  event,
}: {
  eventId: string;
  event: TimelineEventRow;
}) {
  const openCurrent = useTabs((state) => state.openCurrent);

  const openNote = useMutation({
    mutationFn: () =>
      getOrCreateSessionForEventId(eventId, event.title || "Untitled"),
    onSuccess: (sessionId) => {
      openCurrent({ type: "sessions", id: sessionId });
    },
    onError: (error) => {
      console.error("[calendar] failed to open event note", error);
    },
  });

  const seriesId = event.recurrence_series_id ?? "";
  const rules = useSeriesFolderRules();
  const ruleFolder = seriesId
    ? (rules.find((rule) => rule.series_id === seriesId)?.folder_path ?? null)
    : null;
  const { data: linkedSession = null } = useLiveQuery<
    LinkedSessionSqlRow,
    LinkedSessionSqlRow | null
  >({
    sql: `
      SELECT id, folder_path
      FROM sessions
      WHERE event_id = ? AND deleted_at IS NULL
      LIMIT 1
    `,
    params: [eventId],
    mapRows: (rows) => rows[0] ?? null,
  });

  const [view, setView] = useState<"main" | "folders" | "manage">("main");
  const [filing, setFiling] = useState(false);
  const [pickerSessionId, setPickerSessionId] = useState<string | null>(null);
  const [pickerMode, setPickerMode] = useState<"occurrence" | "rule">(
    "occurrence",
  );
  const [folderAtPickerOpen, setFolderAtPickerOpen] = useState("");
  const [pickerClosed, setPickerClosed] = useState(false);
  const declinedSeries = useRef<Set<string>>(new Set());

  const sessionFolder = linkedSession?.folder_path ?? "";
  const displayFolder = sessionFolder || ruleFolder || "";
  const activeView = view === "manage" && !ruleFolder ? "main" : view;
  const showAutoAdd =
    activeView === "main" &&
    seriesId !== "" &&
    ruleFolder === null &&
    pickerClosed &&
    sessionFolder !== "" &&
    sessionFolder !== folderAtPickerOpen &&
    !declinedSeries.current.has(seriesId);

  const openFolderPicker = useCallback(
    async (mode: "occurrence" | "rule") => {
      setFiling(true);
      try {
        const sessionId = await getOrCreateSessionForEventId(
          eventId,
          event.title || "Untitled",
        );
        setPickerSessionId(sessionId);
        setPickerMode(mode);
        setFolderAtPickerOpen(linkedSession?.folder_path ?? "");
        setPickerClosed(false);
        setView("folders");
      } catch (error) {
        console.error("[calendar] failed to open folder picker", error);
      } finally {
        setFiling(false);
      }
    },
    [eventId, event.title, linkedSession?.folder_path],
  );

  const handleFolderButton = useCallback(() => {
    if (filing) return;
    if (ruleFolder) {
      setView("manage");
      return;
    }
    void openFolderPicker("occurrence");
  }, [filing, ruleFolder, openFolderPicker]);

  const handlePickerClose = useCallback(() => {
    setPickerClosed(true);
    setView("main");
  }, []);

  const handleRuleFolderSelect = useCallback(
    async (folderPath: string) => {
      if (!seriesId) return;
      if (folderPath) {
        await setSeriesFolderRuleAndAlign(seriesId, folderPath);
      } else {
        await clearSeriesFolderRule(seriesId);
      }
    },
    [seriesId],
  );

  const handleAutoAdd = useCallback(async () => {
    if (!seriesId || !sessionFolder) return;
    try {
      await setSeriesFolderRuleAndAlign(seriesId, sessionFolder);
    } catch (error) {
      console.error("[calendar] failed to set series folder rule", error);
    } finally {
      setPickerClosed(false);
    }
  }, [seriesId, sessionFolder]);

  const handleDeclineAutoAdd = useCallback(() => {
    if (seriesId) {
      declinedSeries.current.add(seriesId);
    }
    setPickerClosed(false);
  }, [seriesId]);

  const handleStopAutoAdd = useCallback(async () => {
    if (!seriesId) return;
    try {
      await clearSeriesFolderRule(seriesId);
    } catch (error) {
      console.error("[calendar] failed to clear series folder rule", error);
    } finally {
      setView("main");
    }
  }, [seriesId]);

  if (activeView === "folders") {
    return (
      <div className="flex flex-col gap-1 p-2">
        <button
          type="button"
          onClick={() => setView("main")}
          className={cn([
            "text-muted-foreground hover:text-foreground flex items-center gap-1 px-2 py-1 text-xs transition-colors",
          ])}
        >
          <CaretLeft className="size-3.5" aria-hidden="true" />
          {t`Back`}
        </button>
        {pickerSessionId ? (
          pickerMode === "rule" ? (
            <FolderPickerContent
              sessionId={pickerSessionId}
              onClose={handlePickerClose}
              selectedPath={ruleFolder ?? ""}
              onSelectFolder={handleRuleFolderSelect}
            />
          ) : (
            <FolderPickerContent
              sessionId={pickerSessionId}
              onClose={handlePickerClose}
            />
          )
        ) : null}
      </div>
    );
  }

  if (activeView === "manage" && ruleFolder) {
    return (
      <div className="flex flex-col gap-3 p-4">
        <button
          type="button"
          onClick={() => setView("main")}
          className={cn([
            "text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors",
          ])}
        >
          <CaretLeft className="size-3.5" aria-hidden="true" />
          {t`Back`}
        </button>
        <div className="flex items-center justify-between gap-2 text-sm">
          <span className="text-muted-foreground">{t`This note`}</span>
          <span className="min-w-0 truncate font-medium">
            {sessionFolder ? folderDisplayName(sessionFolder) : t`Unfiled`}
          </span>
        </div>
        <div className="flex items-center justify-between gap-2 text-sm">
          <span className="text-muted-foreground flex items-center gap-1.5">
            <Repeat className="size-3.5" aria-hidden="true" />
            {t`Future meetings`}
          </span>
          <span className="min-w-0 truncate font-medium">
            {folderDisplayName(ruleFolder)}
          </span>
        </div>
        <Button
          size="sm"
          variant="secondary"
          className="min-h-8 w-full"
          disabled={filing}
          onClick={() => void openFolderPicker("occurrence")}
        >
          {t`Move this note`}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          className="min-h-8 w-full"
          disabled={filing}
          onClick={() => void openFolderPicker("rule")}
        >
          {t`Change auto-add folder`}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          className="min-h-8 w-full"
          onClick={() => void handleStopAutoAdd()}
        >
          {t`Stop auto-add`}
        </Button>
      </div>
    );
  }

  if (showAutoAdd) {
    return (
      <div className="flex flex-col gap-3 p-4">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Repeat className="size-4 shrink-0" aria-hidden="true" />
          {t`Auto-add future meetings?`}
        </div>
        <p className="text-muted-foreground text-sm">
          {t`Automatically put all future instances of this recurring meeting into a folder.`}
        </p>
        <div className="flex gap-2">
          <Button
            size="sm"
            className="min-h-8 flex-1"
            onClick={() => void handleAutoAdd()}
          >
            {t`Auto-add`}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            className="min-h-8 flex-1"
            onClick={handleDeclineAutoAdd}
          >
            {t`No`}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 p-4">
      <EventDisplay
        event={{
          title: event.title || t`Busy`,
          startedAt: event.started_at ?? undefined,
          endedAt: event.ended_at ?? undefined,
          location: event.location ?? undefined,
          meetingLink: event.meeting_link ?? undefined,
          description: event.description ?? undefined,
          calendarId: event.calendar_id ?? undefined,
        }}
      />
      <Button
        size="sm"
        variant="secondary"
        className="min-h-8 w-full"
        disabled={filing}
        onClick={handleFolderButton}
      >
        {filing ? (
          <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
        ) : (
          <Folder className="size-3.5" aria-hidden="true" />
        )}
        <span className="min-w-0 flex-1 truncate text-left">
          {displayFolder ? folderDisplayName(displayFolder) : t`Add to folder`}
        </span>
        <CaretDown className="size-3.5 shrink-0" aria-hidden="true" />
      </Button>
      <Button
        size="sm"
        className="bg-primary text-primary-foreground hover:bg-primary/90 min-h-8 w-full"
        disabled={openNote.isPending}
        onClick={() => openNote.mutate()}
      >
        {openNote.isPending ? (
          <CircleNotch className="size-3.5 animate-spin" aria-hidden="true" />
        ) : null}
        Open note
      </Button>
    </div>
  );
}
