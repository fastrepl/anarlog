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
import { useIsSessionEnhancing } from "~/session/hooks/useEnhancedNotes";
import { getSessionEvent } from "~/session/utils";
import { openStandaloneNoteWindow } from "~/session/window";
import type { MenuItemDef } from "~/shared/hooks/useNativeContextMenu";
import { InteractiveButton } from "~/shared/ui/interactive-button";
import { useIgnoredEvents } from "~/store/tinybase/hooks";
import {
  captureSessionData,
  deleteSessionCascade,
  finalizeSessionDeletion,
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
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
  }: {
    item: TimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    flatItemKeys: string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
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
          selectedNodeRef={selectedNodeRef}
          itemNodeRef={itemNodeRef}
          isUpcoming={isUpcoming}
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
        selectedNodeRef={selectedNodeRef}
        itemNodeRef={itemNodeRef}
        isUpcoming={isUpcoming}
      />
    );
  },
);

function ItemBase({
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
}: {
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
}) {
  const { t } = useLingui();
  const hasSelection = useTimelineSelection((s) => s.selectedIds.length > 0);
  const showLiveStop = isLive && onStop;
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
          showTrailingStatus && "pr-10",
          ignored ? "cursor-default" : "cursor-pointer",
          multiSelected && "bg-accent",
          !multiSelected && selected && "bg-accent",
          !multiSelected && !selected && "hover:bg-accent/50",
          isUpcoming &&
            !isLive && [
              "bg-destructive/8 text-foreground motion-safe:animate-pulse",
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
}

const EventItem = memo(
  ({
    item,
    precision,
    selected,
    timezone,
    multiSelected,
    flatItemKeys,
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
  }: {
    item: EventTimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    flatItemKeys: string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
  }) => {
    const { t } = useLingui();
    const store = main.UI.useStore(main.STORE_ID);
    const openCurrent = useTabs((state) => state.openCurrent);
    const openNew = useTabs((state) => state.openNew);

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
    const muted = isTimelineItemInFuture(item);

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

    const handleOpenNewTab = useCallback(() => {
      openEvent(true);
    }, [openEvent]);

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
          id: "open-new-tab",
          text: t`Open in New Tab`,
          action: handleOpenNewTab,
        },
        { separator: true as const },
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
      handleOpenNewTab,
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
    selectedNodeRef,
    itemNodeRef,
    isUpcoming,
  }: {
    item: SessionTimelineItem;
    precision: TimelinePrecision;
    selected: boolean;
    timezone?: string;
    multiSelected: boolean;
    flatItemKeys: string[];
    selectedNodeRef?: RefCallback<HTMLDivElement>;
    itemNodeRef?: RefCallback<HTMLDivElement>;
    isUpcoming?: boolean;
  }) => {
    const { t } = useLingui();
    const store = main.UI.useStore(main.STORE_ID);
    const indexes = main.UI.useIndexes(main.STORE_ID);
    const openCurrent = useTabs((state) => state.openCurrent);
    const invalidateResource = useTabs((state) => state.invalidateResource);
    const addDeletion = useUndoDelete((state) => state.addDeletion);
    const { ignoreEvent } = useIgnoredEvents();

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
      useTimelineSelection.getState().selectRange(flatItemKeys, itemKey);
    }, [flatItemKeys, itemKey]);

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
      if (!store) {
        return;
      }

      if (sessionEvent?.tracking_id) {
        ignoreEvent(sessionEvent.tracking_id);
      }

      const capturedData = captureSessionData(store, indexes, sessionId);

      invalidateResource("sessions", sessionId);
      void deleteSessionCascade(store, indexes, sessionId, {
        deferFilesystemDelete: true,
      });

      if (capturedData) {
        addDeletion(capturedData, () => {
          void finalizeSessionDeletion(sessionId);
        });
      }
    }, [
      store,
      indexes,
      sessionId,
      sessionEvent,
      ignoreEvent,
      invalidateResource,
      addDeletion,
    ]);

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
