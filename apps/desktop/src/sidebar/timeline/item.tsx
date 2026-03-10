import { memo, useCallback, useMemo } from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn, format, getYear, safeParseDate, TZDate } from "@hypr/utils";

import {
  type EventTimelineItem,
  type SessionTimelineItem,
  type TimelineItem,
  TimelinePrecision,
} from "./utils";

import { SessionPreviewCard } from "~/session/components/session-preview-card";
import { useIsSessionEnhancing } from "~/session/hooks/useEnhancedNotes";
import { getSessionEvent } from "~/session/utils";
import type { MenuItemDef } from "~/shared/hooks/useNativeContextMenu";
import { InteractiveButton } from "~/shared/ui/interactive-button";
import { useIgnoredEvents } from "~/store/tinybase/hooks";
import {
  captureSessionData,
  deleteSessionCascade,
} from "~/store/tinybase/store/deleteSession";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";
import { useSessionTitle } from "~/store/zustand/live-title";
import { type TabInput, useTabs } from "~/store/zustand/tabs";
import { useTimelineSelection } from "~/store/zustand/timeline-selection";
import { useUndoDelete } from "~/store/zustand/undo-delete";
import { useListener } from "~/stt/contexts";

export const TimelineItemComponent = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    flatItemKeys,
  }: {
    item: TimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    flatItemKeys: string[];
  }) => {
    if (item.type === "event") {
      return (
        <EventItem
          item={item}
          precision={precision}
          selected={selected}
          timezone={timezone}
          multiSelected={multiSelected}
          flatItemKeys={flatItemKeys}
        />
      );
    }
    return (
      <SessionItem
        item={item}
        precision={precision}
        selected={selected}
        timezone={timezone}
        multiSelected={multiSelected}
        flatItemKeys={flatItemKeys}
      />
    );
  },
);

function ItemBase({
  title,
  displayTime,
  calendarId,
  showSpinner,
  selected,
  ignored,
  multiSelected,
  onClick,
  onCmdClick,
  onShiftClick,
  contextMenu,
}: {
  title: string;
  displayTime: string;
  calendarId: string | null;
  showSpinner?: boolean;
  selected: boolean;
  ignored?: boolean;
  multiSelected: boolean;
  onClick: () => void;
  onCmdClick: () => void;
  onShiftClick: () => void;
  contextMenu: MenuItemDef[];
}) {
  const hasSelection = useTimelineSelection((s) => s.selectedIds.length > 0);

  return (
    <InteractiveButton
      onClick={onClick}
      onCmdClick={onCmdClick}
      onShiftClick={onShiftClick}
      contextMenu={hasSelection ? undefined : contextMenu}
      className={cn([
        "w-full cursor-pointer rounded-lg px-3 py-2 text-left",
        multiSelected && "bg-neutral-200",
        !multiSelected && selected && "bg-neutral-200",
        !multiSelected && !selected && "hover:bg-neutral-200/50",
        ignored && "opacity-40",
      ])}
    >
      <div className="flex items-center gap-2">
        {showSpinner && (
          <div className="shrink-0">
            <Spinner size={14} />
          </div>
        )}
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <div
            className={cn(
              "pointer-events-none truncate text-sm font-normal",
              ignored && "line-through",
            )}
          >
            {title || <span className="text-neutral-400">Untitled</span>}
          </div>
          {displayTime && (
            <div className="font-mono text-xs text-neutral-500">
              {displayTime}
            </div>
          )}
        </div>
        {calendarId && <CalendarIndicator calendarId={calendarId} />}
      </div>
    </InteractiveButton>
  );
}

const EventItem = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    flatItemKeys,
  }: {
    item: EventTimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    flatItemKeys: string[];
  }) => {
    const store = main.UI.useStore(main.STORE_ID);
    const openCurrent = useTabs((state) => state.openCurrent);
    const openNew = useTabs((state) => state.openNew);

    const eventId = item.id;
    const trackingIdEvent = item.data.tracking_id_event;
    const title = item.data.title || "Untitled";
    const calendarId = item.data.calendar_id ?? null;
    const recurrenceSeriesId = item.data.recurrence_series_id;

    const {
      isIgnored,
      ignoreEvent,
      unignoreEvent,
      ignoreSeries,
      unignoreSeries,
    } = useIgnoredEvents();

    const ignored = isIgnored(trackingIdEvent, recurrenceSeriesId);

    const displayTime = useMemo(
      () => formatDisplayTime(item.data.started_at, precision, timezone),
      [item.data.started_at, precision, timezone],
    );

    const openEvent = useCallback(
      (openInNewTab: boolean) => {
        if (!store || !eventId) {
          return;
        }

        const sessionId = getOrCreateSessionForEventId(store, eventId, title);
        const tab: TabInput = { id: sessionId, type: "sessions" };
        openInNewTab ? openNew(tab) : openCurrent(tab);
      },
      [eventId, store, title, openCurrent, openNew],
    );

    const itemKey = `event-${item.id}`;

    const handleClick = useCallback(() => {
      useTimelineSelection.getState().setAnchor(itemKey);
      openEvent(false);
    }, [openEvent, itemKey]);

    const handleCmdClick = useCallback(() => {
      useTimelineSelection.getState().toggleSelect(itemKey);
    }, [itemKey]);

    const handleShiftClick = useCallback(() => {
      useTimelineSelection.getState().selectRange(flatItemKeys, itemKey);
    }, [flatItemKeys, itemKey]);

    const handleIgnore = useCallback(() => {
      if (!trackingIdEvent) return;
      ignoreEvent(trackingIdEvent);
    }, [trackingIdEvent, ignoreEvent]);

    const handleUnignore = useCallback(() => {
      if (!trackingIdEvent) return;
      unignoreEvent(trackingIdEvent);
    }, [trackingIdEvent, unignoreEvent]);

    const handleUnignoreSeries = useCallback(() => {
      if (!recurrenceSeriesId) return;
      unignoreSeries(recurrenceSeriesId);
    }, [recurrenceSeriesId, unignoreSeries]);

    const handleIgnoreSeries = useCallback(() => {
      if (!recurrenceSeriesId) return;
      ignoreSeries(recurrenceSeriesId);
    }, [recurrenceSeriesId, ignoreSeries]);

    const contextMenu = useMemo(() => {
      if (ignored) {
        if (recurrenceSeriesId) {
          return [
            {
              id: "unignore",
              text: "Unignore Only This Event",
              action: handleUnignore,
            },
            {
              id: "unignore-series",
              text: "Unignore All Recurring Events",
              action: handleUnignoreSeries,
            },
          ];
        }
        return [
          { id: "unignore", text: "Unignore Event", action: handleUnignore },
        ];
      }
      const menu = [
        { id: "ignore", text: "Ignore Event", action: handleIgnore },
      ];
      if (recurrenceSeriesId) {
        menu.push({
          id: "ignore-series",
          text: "Ignore All Recurring Events",
          action: handleIgnoreSeries,
        });
      }
      return menu;
    }, [
      ignored,
      handleIgnore,
      handleUnignore,
      handleUnignoreSeries,
      handleIgnoreSeries,
      recurrenceSeriesId,
    ]);

    return (
      <ItemBase
        title={title}
        displayTime={displayTime}
        calendarId={calendarId}
        selected={selected}
        ignored={ignored}
        multiSelected={multiSelected}
        onClick={handleClick}
        onCmdClick={handleCmdClick}
        onShiftClick={handleShiftClick}
        contextMenu={contextMenu}
      />
    );
  },
);

const SessionItem = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    flatItemKeys,
  }: {
    item: SessionTimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    flatItemKeys: string[];
  }) => {
    const store = main.UI.useStore(main.STORE_ID);
    const indexes = main.UI.useIndexes(main.STORE_ID);
    const openCurrent = useTabs((state) => state.openCurrent);
    const openNew = useTabs((state) => state.openNew);
    const invalidateResource = useTabs((state) => state.invalidateResource);
    const addDeletion = useUndoDelete((state) => state.addDeletion);

    const sessionId = item.id;
    const storeTitle = main.UI.useCell(
      "sessions",
      sessionId,
      "title",
      main.STORE_ID,
    ) as string | undefined;
    const title = useSessionTitle(sessionId, storeTitle);

    const sessionMode = useListener((state) => state.getSessionMode(sessionId));
    const isEnhancing = useIsSessionEnhancing(sessionId);
    const isFinalizing = sessionMode === "finalizing";
    const isBatching = sessionMode === "running_batch";
    const showSpinner =
      !selected && (isFinalizing || isEnhancing || isBatching);

    const sessionEvent = useMemo(
      () => getSessionEvent(item.data),
      [item.data.event_json],
    );

    const calendarId = sessionEvent?.calendar_id ?? null;
    const hasEvent = !!item.data.event_json;

    const displayTime = useMemo(
      () =>
        formatDisplayTime(
          sessionEvent?.started_at ?? item.data.created_at,
          precision,
          timezone,
        ),
      [sessionEvent?.started_at, item.data.created_at, precision, timezone],
    );

    const itemKey = `session-${item.id}`;

    const handleClick = useCallback(() => {
      useTimelineSelection.getState().setAnchor(itemKey);
      openCurrent({ id: sessionId, type: "sessions" });
    }, [sessionId, openCurrent, itemKey]);

    const handleCmdClick = useCallback(() => {
      useTimelineSelection.getState().toggleSelect(itemKey);
    }, [itemKey]);

    const handleShiftClick = useCallback(() => {
      useTimelineSelection.getState().selectRange(flatItemKeys, itemKey);
    }, [flatItemKeys, itemKey]);

    const handleOpenNewTab = useCallback(() => {
      openNew({ id: sessionId, type: "sessions" });
    }, [sessionId, openNew]);

    const handleDelete = useCallback(() => {
      if (!store) {
        return;
      }

      const capturedData = captureSessionData(store, indexes, sessionId);

      invalidateResource("sessions", sessionId);
      void deleteSessionCascade(store, indexes, sessionId, {
        skipAudio: true,
      });

      if (capturedData) {
        addDeletion(capturedData, () => {
          void fsSyncCommands.audioDelete(sessionId);
        });
      }
    }, [store, indexes, sessionId, invalidateResource, addDeletion]);

    const handleShowInFinder = useCallback(async () => {
      const result = await fsSyncCommands.sessionDir(sessionId);
      if (result.status === "ok") {
        await openerCommands.openPath(result.data, null);
      }
    }, [sessionId]);

    const contextMenu = useMemo(
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
        { separator: true as const },
        {
          id: "delete",
          text: hasEvent ? "Delete Attached Note" : "Delete Note",
          action: handleDelete,
        },
      ],
      [handleOpenNewTab, handleShowInFinder, handleDelete, hasEvent],
    );

    return (
      <SessionPreviewCard
        sessionId={sessionId}
        side="right"
        enabled={!selected}
      >
        <ItemBase
          title={title}
          displayTime={displayTime}
          calendarId={calendarId}
          showSpinner={showSpinner}
          selected={selected}
          multiSelected={multiSelected}
          onClick={handleClick}
          onCmdClick={handleCmdClick}
          onShiftClick={handleShiftClick}
          contextMenu={contextMenu}
        />
      </SessionPreviewCard>
    );
  },
);

function formatDisplayTime(
  timestamp: string | null | undefined,
  precision: TimelinePrecision,
  timezone?: string,
): string {
  const parsed = safeParseDate(timestamp);
  if (!parsed) {
    return "";
  }

  const date = timezone ? new TZDate(parsed, timezone) : parsed;
  const time = format(date, "h:mm a").toUpperCase();

  if (precision === "time") {
    return time;
  }

  const now = timezone ? new TZDate(new Date(), timezone) : new Date();
  const sameYear = getYear(date) === getYear(now);
  const dateStr = sameYear
    ? format(date, "MMM d")
    : format(date, "MMM d, yyyy");

  return `${dateStr}, ${time}`;
}

function CalendarIndicator({ calendarId }: { calendarId: string }) {
  const calendar = main.UI.useRow("calendars", calendarId, main.STORE_ID);

  const name = calendar?.name ? String(calendar.name) : undefined;
  const color = calendar?.color ? String(calendar.color) : "#888";

  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <div
          className="size-2 shrink-0 rounded-full opacity-60"
          style={{ backgroundColor: color }}
        />
      </TooltipTrigger>
      <TooltipContent side="right" className="text-xs">
        {name || "Calendar"}
      </TooltipContent>
    </Tooltip>
  );
}
