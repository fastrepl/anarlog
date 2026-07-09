import { useLingui } from "@lingui/react/macro";
import { SquareIcon } from "lucide-react";
import {
  memo,
  type DragEvent,
  type RefCallback,
  useCallback,
  useMemo,
} from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn, format, getYear, safeParseDate, TZDate } from "@hypr/utils";

import {
  type EventTimelineItem,
  isTimelineItemInFuture,
  type SessionTimelineItem,
  type TimelineItem,
  TimelinePrecision,
} from "./utils";

import { writeSessionContextDragData } from "~/chat/context/session-drag";
import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { useIsSessionEnhancing } from "~/session/hooks/useEnhancedNotes";
import { getSessionEvent } from "~/session/utils";
import { openStandaloneNoteWindow } from "~/session/window";
import type { MenuItemDef } from "~/shared/hooks/useNativeContextMenu";
import { InteractiveButton } from "~/shared/ui/interactive-button";
import { useIgnoredEvents } from "~/store/tinybase/hooks";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";
import { useSessionTitle } from "~/store/zustand/live-title";
import { useTabs } from "~/store/zustand/tabs";
import { useTimelineSelection } from "~/store/zustand/timeline-selection";
import { useListener } from "~/stt/contexts";

const EMPTY_TIMELINE_ITEM_KEYS: string[] = [];

type ItemBaseProps = {
  title: string;
  displayTime: string;
  isLive?: boolean;
  amplitude?: number;
  showSpinner?: boolean;
  selected: boolean;
  ignored?: boolean;
  muted?: boolean;
  multiSelected: boolean;
  onClick: () => void;
  onDoubleClick?: () => void;
  onCmdClick: () => void;
  onShiftClick: () => void;
  onStop?: () => void;
  onDragStart?: (event: DragEvent<HTMLElement>) => void;
  contextMenu: MenuItemDef[];
  draggable?: boolean;
  selectedNodeRef?: RefCallback<HTMLDivElement>;
  itemNodeRef?: RefCallback<HTMLDivElement>;
  timelineSessionId?: string;
  isUpcoming?: boolean;
  upcomingProgress?: number;
};

export const TimelineItemComponent = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    flatItemKeys,
    getFlatItemKeys,
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
    upcomingProgress,
  }: {
    item: TimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    flatItemKeys?: string[];
    getFlatItemKeys?: () => string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
    upcomingLabel?: string;
    upcomingProgress?: number;
  }) => {
    const readFlatItemKeys =
      getFlatItemKeys ?? (() => flatItemKeys ?? EMPTY_TIMELINE_ITEM_KEYS);

    if (item.type === "event") {
      return (
        <EventItem
          item={item}
          precision={precision}
          selected={selected}
          timezone={timezone}
          multiSelected={multiSelected}
          getFlatItemKeys={readFlatItemKeys}
          selectedNodeRef={selectedNodeRef}
          itemNodeRef={itemNodeRef}
          isUpcoming={isUpcoming}
          upcomingProgress={upcomingProgress}
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
        getFlatItemKeys={readFlatItemKeys}
        selectedNodeRef={selectedNodeRef}
        itemNodeRef={itemNodeRef}
        isUpcoming={isUpcoming}
        upcomingProgress={upcomingProgress}
      />
    );
  },
);

const ItemBase = memo(function ItemBase({
  title,
  displayTime,
  isLive,
  amplitude,
  showSpinner,
  selected,
  ignored,
  muted,
  multiSelected,
  onClick,
  onDoubleClick,
  onCmdClick,
  onShiftClick,
  onStop,
  onDragStart,
  contextMenu,
  draggable,
  selectedNodeRef,
  itemNodeRef,
  timelineSessionId,
  isUpcoming,
  upcomingProgress,
}: ItemBaseProps) {
  const { t } = useLingui();
  const hasSelection = useTimelineSelection((s) => s.selectedIds.length > 0);
  const showLiveStop = isLive && onStop;
  const showUpcomingGauge =
    typeof upcomingProgress === "number" &&
    Boolean(isUpcoming) &&
    !isLive &&
    !showSpinner;
  const upcomingGaugePercent =
    typeof upcomingProgress === "number"
      ? Math.round(Math.max(0, Math.min(upcomingProgress, 1)) * 100)
      : 0;
  const showTrailingStatus = showLiveStop || showSpinner;
  const setItemRef = useCallback(
    (node: HTMLDivElement | null) => {
      selectedNodeRef?.(node);
      itemNodeRef?.(node);
    },
    [selectedNodeRef, itemNodeRef],
  );

  return (
    <div
      ref={setItemRef}
      data-sidebar-timeline-session-id={timelineSessionId}
      className="group/sidebar-live-item relative"
    >
      <InteractiveButton
        onClick={ignored ? undefined : onClick}
        onDoubleClick={ignored ? undefined : onDoubleClick}
        onCmdClick={ignored ? undefined : onCmdClick}
        onShiftClick={ignored ? undefined : onShiftClick}
        onDragStart={onDragStart}
        contextMenu={hasSelection ? undefined : contextMenu}
        className={cn([
          "w-full rounded-lg px-3 py-2 text-left",
          showUpcomingGauge && "pl-4",
          showTrailingStatus && "pr-10",
          ignored ? "cursor-default" : "cursor-pointer",
          multiSelected && "bg-accent",
          !multiSelected && selected && "bg-accent",
          !multiSelected && !selected && "hover:bg-accent/50",
          isUpcoming &&
            !isLive && [
              "bg-destructive/8 text-foreground",
              "hover:bg-destructive/12 focus-visible:ring-destructive/25",
            ],
          isLive && [
            "bg-destructive text-destructive-foreground hover:bg-destructive/90",
            "focus-visible:ring-destructive/40 focus-visible:ring-2 focus-visible:outline-hidden",
          ],
          ignored && "opacity-40",
          !ignored && muted && !isLive && !isUpcoming && "opacity-65",
        ])}
        draggable={draggable}
      >
        <div className="flex items-center gap-2">
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <div
              className={cn(
                "pointer-events-none truncate text-sm font-normal",
                ignored && "line-through",
              )}
            >
              {title || t`Untitled`}
            </div>
            {displayTime && (
              <div
                className={cn([
                  "font-mono text-xs",
                  isLive
                    ? "text-destructive-foreground/65"
                    : "text-muted-foreground",
                ])}
              >
                {displayTime}
              </div>
            )}
          </div>
        </div>
      </InteractiveButton>
      {showUpcomingGauge ? (
        <div
          aria-hidden
          data-sidebar-timeline-upcoming-gauge
          className="bg-destructive/20 pointer-events-none absolute top-2 bottom-2 left-1.5 w-0.5 overflow-hidden rounded-full"
        >
          <div
            data-sidebar-timeline-upcoming-gauge-fill
            className="bg-destructive absolute bottom-0 left-0 w-full rounded-full transition-[height] duration-300 ease-linear"
            style={{ height: `${upcomingGaugePercent}%` }}
          />
        </div>
      ) : null}
      {showSpinner ? (
        <div
          aria-hidden
          className="text-muted-foreground pointer-events-none absolute top-1/2 right-3 flex size-5 -translate-y-1/2 items-center justify-center"
        >
          <Spinner size={14} />
        </div>
      ) : null}
      {showLiveStop ? (
        <button
          type="button"
          aria-label={t`Stop listening`}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            onStop();
          }}
          className={cn([
            "absolute top-1/2 right-3 flex size-5 -translate-y-1/2 items-center justify-center rounded-sm",
            "text-white/80 transition-colors hover:bg-white/15 hover:text-white",
            "focus-visible:ring-2 focus-visible:ring-white/70 focus-visible:outline-hidden",
          ])}
        >
          <span
            aria-hidden
            className="flex items-center justify-center group-hover/sidebar-live-item:hidden"
          >
            <DancingSticks
              amplitude={amplitude ?? 0.25}
              color="currentColor"
              height={14}
              width={13}
              stickWidth={2}
              gap={2}
            />
          </span>
          <span
            aria-hidden
            className="hidden items-center justify-center group-hover/sidebar-live-item:flex"
          >
            <SquareIcon size={10} className="fill-current" />
          </span>
        </button>
      ) : null}
    </div>
  );
}, itemBasePropsAreEqual);

function itemBasePropsAreEqual(prev: ItemBaseProps, next: ItemBaseProps) {
  return (
    prev.title === next.title &&
    prev.displayTime === next.displayTime &&
    prev.isLive === next.isLive &&
    prev.amplitude === next.amplitude &&
    prev.showSpinner === next.showSpinner &&
    prev.selected === next.selected &&
    prev.ignored === next.ignored &&
    prev.muted === next.muted &&
    prev.multiSelected === next.multiSelected &&
    prev.onClick === next.onClick &&
    prev.onDoubleClick === next.onDoubleClick &&
    prev.onCmdClick === next.onCmdClick &&
    prev.onShiftClick === next.onShiftClick &&
    prev.onStop === next.onStop &&
    prev.onDragStart === next.onDragStart &&
    prev.contextMenu === next.contextMenu &&
    prev.draggable === next.draggable &&
    prev.selectedNodeRef === next.selectedNodeRef &&
    prev.itemNodeRef === next.itemNodeRef &&
    prev.timelineSessionId === next.timelineSessionId &&
    prev.isUpcoming === next.isUpcoming &&
    prev.upcomingProgress === next.upcomingProgress
  );
}

const EventItem = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    getFlatItemKeys,
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
    upcomingProgress,
  }: {
    item: EventTimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    getFlatItemKeys: () => string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
    upcomingProgress?: number;
  }) => {
    const { t } = useLingui();
    const store = main.UI.useStore(main.STORE_ID);
    const openCurrent = useTabs((state) => state.openCurrent);

    const eventId = item.id;
    const trackingIdEvent = item.data.tracking_id_event;
    const title = item.data.title || t`Untitled`;
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

    const openEvent = useCallback(() => {
      if (!store || !eventId) {
        return;
      }

      const sessionId = getOrCreateSessionForEventId(store, eventId, title);
      openCurrent({ id: sessionId, type: "sessions" });
    }, [eventId, store, title, openCurrent]);

    const itemKey = `event-${item.id}`;
    const muted = isTimelineItemInFuture(item);

    const handleClick = useCallback(() => {
      useTimelineSelection.getState().setAnchor(itemKey);
      openEvent();
    }, [openEvent, itemKey]);

    const handleCmdClick = useCallback(() => {
      useTimelineSelection.getState().toggleSelect(itemKey);
    }, [itemKey]);

    const handleShiftClick = useCallback(() => {
      useTimelineSelection.getState().selectRange(getFlatItemKeys(), itemKey);
    }, [getFlatItemKeys, itemKey]);

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
              text: t`Show This Event`,
              action: handleUnignore,
            },
            {
              id: "unignore-series",
              text: t`Show All Recurring Events`,
              action: handleUnignoreSeries,
            },
          ];
        }
        return [
          { id: "unignore", text: t`Show Event`, action: handleUnignore },
        ];
      }
      const menu: MenuItemDef[] = [
        {
          id: "ignore",
          text: recurrenceSeriesId ? t`Delete This Event` : t`Delete Event`,
          action: handleIgnore,
        },
      ];
      if (recurrenceSeriesId) {
        menu.push({
          id: "ignore-series",
          text: t`Delete All Recurring Events`,
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
      t,
    ]);

    return (
      <ItemBase
        title={title}
        displayTime={displayTime}
        selected={selected}
        ignored={ignored}
        muted={muted}
        multiSelected={multiSelected}
        onClick={handleClick}
        onCmdClick={handleCmdClick}
        onShiftClick={handleShiftClick}
        contextMenu={contextMenu}
        selectedNodeRef={selected ? selectedNodeRef : undefined}
        itemNodeRef={itemNodeRef}
        isUpcoming={isUpcoming}
        upcomingProgress={upcomingProgress}
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
    getFlatItemKeys,
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
    upcomingProgress,
  }: {
    item: SessionTimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    getFlatItemKeys: () => string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
    upcomingProgress?: number;
  }) => {
    const { t } = useLingui();
    const openCurrent = useTabs((state) => state.openCurrent);
    const deleteSession = useDeleteSession();

    const sessionId = item.id;
    const storeTitle = main.UI.useCell(
      "sessions",
      sessionId,
      "title",
      main.STORE_ID,
    ) as string | undefined;
    const title = useSessionTitle(sessionId, storeTitle);

    const { sessionMode, stop, amplitude } = useListener((state) => {
      const sessionMode = state.getSessionMode(sessionId);
      return {
        sessionMode,
        stop: state.stop,
        amplitude: sessionMode === "active" ? state.live.amplitude : null,
      };
    });
    const isEnhancing = useIsSessionEnhancing(sessionId);
    const isLive = sessionMode === "active";
    const isFinalizing = sessionMode === "finalizing";
    const isBatching = sessionMode === "running_batch";
    const showSpinner =
      !selected && !isLive && (isFinalizing || isEnhancing || isBatching);

    const sessionEvent = useMemo(
      () => getSessionEvent(item.data),
      [item.data.event_json],
    );

    const displayTime = useMemo(
      () =>
        formatDisplayTime(
          sessionEvent?.started_at ?? item.data.created_at,
          precision,
          timezone,
        ),
      [sessionEvent?.started_at, item.data.created_at, precision, timezone],
    );
    const muted = isTimelineItemInFuture(item);

    const itemKey = `session-${item.id}`;

    const handleClick = useCallback(() => {
      useTimelineSelection.getState().setAnchor(itemKey);
      openCurrent({ id: sessionId, type: "sessions" });
    }, [sessionId, openCurrent, itemKey]);

    const handleCmdClick = useCallback(() => {
      useTimelineSelection.getState().toggleSelect(itemKey);
    }, [itemKey]);

    const handleShiftClick = useCallback(() => {
      useTimelineSelection.getState().selectRange(getFlatItemKeys(), itemKey);
    }, [getFlatItemKeys, itemKey]);

    const handleOpenStandaloneWindow = useCallback(() => {
      void openStandaloneNoteWindow(sessionId);
    }, [sessionId]);

    const handleDragStart = useCallback(
      (event: DragEvent<HTMLElement>) => {
        writeSessionContextDragData(
          event.dataTransfer,
          sessionId,
          title || t`Untitled`,
        );
      },
      [sessionId, title, t],
    );

    const handleDelete = useCallback(() => {
      deleteSession(sessionId, sessionEvent?.tracking_id);
    }, [deleteSession, sessionId, sessionEvent?.tracking_id]);

    const handleShowInFinder = useCallback(async () => {
      const result = await fsSyncCommands.sessionDir(sessionId);
      if (result.status === "ok") {
        await openerCommands.openPath(result.data, null);
      }
    }, [sessionId]);

    const contextMenu = useMemo(
      () => [
        {
          id: "open-new-window",
          text: t`Open in New Window`,
          action: handleOpenStandaloneWindow,
        },
        {
          id: "show",
          text: t`Show in Finder`,
          action: handleShowInFinder,
        },
        { separator: true as const },
        {
          id: "delete",
          text: t`Delete Note`,
          action: handleDelete,
        },
      ],
      [handleOpenStandaloneWindow, handleShowInFinder, handleDelete, t],
    );

    return (
      <ItemBase
        title={title}
        displayTime={displayTime}
        isLive={isLive}
        amplitude={Math.max(
          0.25,
          Math.min(Math.hypot(amplitude?.mic ?? 0, amplitude?.speaker ?? 0), 1),
        )}
        showSpinner={showSpinner}
        selected={selected}
        muted={muted}
        multiSelected={multiSelected}
        onClick={handleClick}
        onDoubleClick={handleOpenStandaloneWindow}
        onCmdClick={handleCmdClick}
        onShiftClick={handleShiftClick}
        onStop={stop}
        onDragStart={handleDragStart}
        contextMenu={contextMenu}
        selectedNodeRef={selected ? selectedNodeRef : undefined}
        itemNodeRef={itemNodeRef}
        timelineSessionId={sessionId}
        isUpcoming={isUpcoming}
        upcomingProgress={upcomingProgress}
        draggable
      />
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
