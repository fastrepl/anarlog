import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  CalendarIcon,
  PlusIcon,
  SquareIcon,
  SunIcon,
} from "lucide-react";
import {
  memo,
  type CSSProperties,
  type DragEvent,
  type MouseEvent,
  type MouseEventHandler,
  type PointerEvent,
  type ReactNode,
  type UIEvent,
  useCallback,
  useMemo,
  useRef,
  useState,
} from "react";

import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  addDays,
  cn,
  differenceInCalendarDays,
  format,
  safeParseDate,
  startOfDay,
  TZDate,
} from "@hypr/utils";

import {
  normalizeEndMs,
  TIMELINE_BLOCK_MS,
  type MeetingTimelineEntry,
} from "./meeting-timeline-layout";
import {
  buildSessionRecordingRanges,
  type SessionRecordingRange,
  type TimelineTranscriptsTable,
} from "./meeting-timeline-recordings";

import { writeSessionContextDragData } from "~/chat/context/session-drag";
import { SessionPreviewCard } from "~/session/components/session-preview-card";
import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { useIsSessionEnhancing } from "~/session/hooks/useEnhancedNotes";
import { getSessionEvent } from "~/session/utils";
import { useConfigValue } from "~/shared/config";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { useNewNote } from "~/shared/useNewNote";
import { useCurrentTimeMs } from "~/sidebar/timeline/realtime";
import type {
  TimelineEventRow,
  TimelineEventsTable,
  TimelineSessionRow,
  TimelineSessionsTable,
} from "~/sidebar/timeline/utils";
import { useIgnoredEvents } from "~/store/tinybase/hooks";
import * as main from "~/store/tinybase/store/main";
import { getOrCreateSessionForEventId } from "~/store/tinybase/store/sessions";
import { useSessionTitle } from "~/store/zustand/live-title";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

const TIMELINE_HEIGHT = 44;
const TIMELINE_CAROUSEL_CARD_WIDTH = 160;
const TIMELINE_CAROUSEL_PADDING = 0;
const TIMELINE_CAROUSEL_END_PADDING = 24;
const TIMELINE_CAROUSEL_GAP = 4;
const TIMELINE_PAST_DAYS = 6;
const TIMELINE_FUTURE_DAYS = 1;
const TIMELINE_WINDOW_DRAG_THRESHOLD_PX = 5;
type TodayChipDirection = "left" | "right";

type TimelineWindowDragStart = {
  pointerId: number;
  clientX: number;
  clientY: number;
  dragging: boolean;
};

export function TopMeetingTimeline({ currentTab }: { currentTab: Tab | null }) {
  const timezone = useConfigValue("timezone") || undefined;
  const scrollContainerRef = useRef<HTMLDivElement | null>(null);
  const detachWheelListenerRef = useRef<(() => void) | null>(null);
  const appliedScrollAnchorRef = useRef<string | null>(null);
  const windowDragStartRef = useRef<TimelineWindowDragStart | null>(null);
  const suppressNextClickRef = useRef(false);

  const selectedSessionId =
    currentTab?.type === "sessions" ? currentTab.id : null;

  const timelineEventsTable = main.UI.useResultTable(
    main.QUERIES.timelineEvents,
    main.STORE_ID,
  ) as TimelineEventsTable;
  const timelineSessionsTable = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  ) as TimelineSessionsTable;
  const transcriptsTable = main.UI.useTable(
    "transcripts",
    main.STORE_ID,
  ) as TimelineTranscriptsTable;

  const { isIgnored } = useIgnoredEvents();
  const liveSessionId = useListener((state) => state.live.sessionId);
  const today = getTimelineDayStart(new Date(), timezone);
  const todayMs = today.getTime();
  const timelineStart = useMemo(
    () => addDays(new Date(todayMs), -TIMELINE_PAST_DAYS),
    [todayMs],
  );
  const timelineEnd = useMemo(
    () => addDays(new Date(todayMs), TIMELINE_FUTURE_DAYS + 1),
    [todayMs],
  );
  const currentTimeMs = useCurrentTimeMs();

  const sessionRecordingRanges = useMemo(
    () => buildSessionRecordingRanges(transcriptsTable),
    [transcriptsTable],
  );

  const entries = useMemo(
    () =>
      buildMeetingTimelineEntries({
        timelineEventsTable,
        timelineSessionsTable,
        sessionRecordingRanges,
        selectedSessionId,
        liveSessionId,
        now: currentTimeMs,
        isIgnored,
      }),
    [
      timelineEventsTable,
      timelineSessionsTable,
      sessionRecordingRanges,
      selectedSessionId,
      liveSessionId,
      currentTimeMs,
      isIgnored,
    ],
  );

  const selectedEntry = useMemo(
    () =>
      selectedSessionId
        ? entries.find(
            (entry) =>
              entry.type === "session" && entry.id === selectedSessionId,
          )
        : null,
    [entries, selectedSessionId],
  );

  const [todayChipDirection, setTodayChipDirection] =
    useState<TodayChipDirection | null>(null);
  const createNewNote = useNewNote({ behavior: "current" });
  const openNew = useTabs((state) => state.openNew);

  const renderItems = useMemo(
    () =>
      buildTimelineRenderItems(entries, {
        startInclusive: timelineStart,
        endExclusive: timelineEnd,
      }),
    [entries, timelineStart, timelineEnd],
  );
  const hasHiddenPastItems = useMemo(
    () => hasTimelineEntriesBefore(entries, timelineStart),
    [entries, timelineStart],
  );
  const hasHiddenFutureItems = useMemo(
    () => hasTimelineEntriesAfter(entries, timelineEnd),
    [entries, timelineEnd],
  );
  const carouselItems = useMemo(
    () =>
      buildTimelineCarouselItems({
        renderItems,
        currentDate: new Date(todayMs),
        startInclusive: timelineStart,
        endExclusive: timelineEnd,
        hasHiddenPastItems,
        hasHiddenFutureItems,
        currentTime: new Date(Math.max(currentTimeMs, Date.now())),
        includeCreateNote: true,
      }),
    [
      renderItems,
      todayMs,
      timelineStart,
      timelineEnd,
      hasHiddenPastItems,
      hasHiddenFutureItems,
      currentTimeMs,
    ],
  );
  const carouselWidth = getTimelineCarouselWidth(carouselItems);
  const nowIndicatorX = useMemo(
    () =>
      getTimelineCarouselNowX(carouselItems, new Date(currentTimeMs), timezone),
    [carouselItems, currentTimeMs, timezone],
  );
  const showNowIndicator = nowIndicatorX !== null && !liveSessionId;
  const openCalendar = useCallback(
    () => openNew({ type: "calendar" }),
    [openNew],
  );

  const scrollAnchorKey = getTimelineCarouselAnchorKey(
    carouselItems,
    selectedSessionId,
  );
  const scrollAnchorMs = selectedEntry?.start.getTime() ?? todayMs;

  const updateTodayChipFromScroll = useCallback(
    (node: HTMLDivElement) => {
      const nextDirection =
        getTimelineCarouselNowDirection({
          nowX: nowIndicatorX,
          scrollLeft: node.scrollLeft,
          viewportWidth: node.clientWidth,
        }) ??
        getTimelineCarouselDateDirection({
          items: carouselItems,
          date: new Date(todayMs),
          timezone,
          scrollLeft: node.scrollLeft,
          viewportWidth: node.clientWidth,
        });

      setTodayChipDirection((previousDirection) =>
        previousDirection === nextDirection ? previousDirection : nextDirection,
      );
    },
    [carouselItems, nowIndicatorX, todayMs, timezone],
  );

  const handleWheel = useCallback(
    (event: WheelEvent) => {
      const node = scrollContainerRef.current;
      if (!node) {
        return;
      }

      if (Math.abs(event.deltaY) > Math.abs(event.deltaX)) {
        event.preventDefault();
        node.scrollLeft += event.deltaY;
        updateTodayChipFromScroll(node);
      }
    },
    [updateTodayChipFromScroll],
  );

  const setScrollContainer = useCallback(
    (node: HTMLDivElement | null) => {
      detachWheelListenerRef.current?.();
      detachWheelListenerRef.current = null;
      scrollContainerRef.current = node;

      if (!node) {
        return;
      }

      node.addEventListener("wheel", handleWheel, { passive: false });
      detachWheelListenerRef.current = () => {
        node.removeEventListener("wheel", handleWheel);
      };

      if (appliedScrollAnchorRef.current === scrollAnchorKey) {
        return;
      }

      const anchorLeft = selectedEntry
        ? getTimelineCarouselX(carouselItems, scrollAnchorMs)
        : getTimelineCarouselDateX(carouselItems, new Date(todayMs), timezone);
      node.scrollLeft = Math.max(0, anchorLeft - node.clientWidth * 0.35);
      updateTodayChipFromScroll(node);
      appliedScrollAnchorRef.current = scrollAnchorKey;
    },
    [
      scrollAnchorKey,
      scrollAnchorMs,
      selectedEntry,
      todayMs,
      timezone,
      carouselItems,
      updateTodayChipFromScroll,
      handleWheel,
    ],
  );

  const handleScroll = useCallback(
    (event: UIEvent<HTMLDivElement>) => {
      updateTodayChipFromScroll(event.currentTarget);
    },
    [updateTodayChipFromScroll],
  );

  const handleGoToToday = useCallback(() => {
    const node = scrollContainerRef.current;
    if (!node) {
      return;
    }

    const todayLeft =
      nowIndicatorX ??
      getTimelineCarouselDateX(carouselItems, new Date(todayMs), timezone);
    node.scrollLeft = Math.max(0, todayLeft - node.clientWidth * 0.5);
    updateTodayChipFromScroll(node);
  }, [
    carouselItems,
    nowIndicatorX,
    todayMs,
    timezone,
    updateTodayChipFromScroll,
  ]);

  const handleTimelineContextMenu = useCallback<
    MouseEventHandler<HTMLDivElement>
  >((event) => {
    event.preventDefault();
    event.stopPropagation();
  }, []);

  const handleTimelinePointerDown = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      suppressNextClickRef.current = false;

      if (event.button !== 0) {
        windowDragStartRef.current = null;
        return;
      }

      windowDragStartRef.current = {
        pointerId: event.pointerId,
        clientX: event.clientX,
        clientY: event.clientY,
        dragging: false,
      };
    },
    [],
  );

  const handleTimelinePointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const dragStart = windowDragStartRef.current;

      if (
        !dragStart ||
        dragStart.dragging ||
        dragStart.pointerId !== event.pointerId ||
        !isTimelineWindowDrag(dragStart, event)
      ) {
        return;
      }

      dragStart.dragging = true;
      suppressNextClickRef.current = true;
      event.preventDefault();

      if (isTauri()) {
        void getCurrentWindow()
          .startDragging()
          .catch(() => {});
      }
    },
    [],
  );

  const handleTimelinePointerEnd = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const dragStart = windowDragStartRef.current;

      if (!dragStart || dragStart.pointerId !== event.pointerId) {
        return;
      }

      windowDragStartRef.current = null;
    },
    [],
  );

  const handleTimelineClickCapture = useCallback(
    (event: MouseEvent<HTMLDivElement>) => {
      if (!suppressNextClickRef.current) {
        return;
      }

      suppressNextClickRef.current = false;
      event.preventDefault();
      event.stopPropagation();
    },
    [],
  );

  return (
    <div
      data-tauri-drag-region
      className="min-w-0 shrink-0 select-none"
      onPointerDown={handleTimelinePointerDown}
      onPointerMove={handleTimelinePointerMove}
      onPointerUp={handleTimelinePointerEnd}
      onPointerCancel={handleTimelinePointerEnd}
      onClickCapture={handleTimelineClickCapture}
    >
      <div className="relative">
        <div
          ref={setScrollContainer}
          onScroll={handleScroll}
          className={cn([
            "scroll-fade-x min-w-0",
            "scrollbar-hide overflow-x-auto overflow-y-hidden",
            "overscroll-contain",
          ])}
          style={{ height: TIMELINE_HEIGHT }}
        >
          <div
            className="group/timeline-strip relative flex h-full min-w-full items-start gap-1"
            onContextMenu={handleTimelineContextMenu}
            style={{
              width: carouselWidth,
            }}
          >
            {showNowIndicator ? (
              <TopCurrentTimeIndicator
                currentTimeMs={currentTimeMs}
                left={nowIndicatorX}
                timezone={timezone}
              />
            ) : null}
            {carouselItems.map((renderItem) =>
              renderItem.kind === "create-note" ? (
                <TimelineCreateNoteCard
                  key={renderItem.id}
                  item={renderItem}
                  onClick={createNewNote}
                />
              ) : renderItem.kind === "open-calendar" ? (
                <TimelineOpenCalendarCard
                  key={renderItem.id}
                  item={renderItem}
                  onClick={openCalendar}
                />
              ) : renderItem.item.type === "session" ? (
                <SessionTimelineBar
                  key={`${renderItem.item.type}-${renderItem.item.id}`}
                  item={renderItem.item}
                  timezone={timezone}
                />
              ) : (
                <EventTimelineBar
                  key={`${renderItem.item.type}-${renderItem.item.id}`}
                  item={renderItem.item}
                  timezone={timezone}
                />
              ),
            )}
          </div>
        </div>
        {todayChipDirection ? (
          <button
            type="button"
            className={cn([
              "absolute top-1/2 z-40 flex h-6 -translate-y-1/2 items-center gap-1 rounded-full border border-neutral-200 bg-white/95 px-2.5 text-xs font-semibold text-neutral-900 shadow-md backdrop-blur",
              "transition-colors hover:border-neutral-300 hover:bg-white hover:text-neutral-950",
              "focus-visible:ring-2 focus-visible:ring-neutral-900 focus-visible:outline-hidden",
              todayChipDirection === "left" ? "left-3" : "right-3",
            ])}
            onClick={handleGoToToday}
          >
            {todayChipDirection === "left" ? <ArrowLeftIcon size={12} /> : null}
            <SunIcon size={13} className="shrink-0 text-yellow-400" />
            <span>Now</span>
            {todayChipDirection === "right" ? (
              <ArrowRightIcon size={12} />
            ) : null}
          </button>
        ) : null}
      </div>
    </div>
  );
}

function TopCurrentTimeIndicator({
  currentTimeMs,
  left,
  timezone,
}: {
  currentTimeMs: number;
  left: number;
  timezone?: string;
}) {
  const label = useMemo(() => {
    const now = timezone
      ? new TZDate(new Date(currentTimeMs), timezone)
      : new Date(currentTimeMs);
    return format(now, "h:mm a").toUpperCase();
  }, [currentTimeMs, timezone]);

  return (
    <div
      aria-hidden
      data-testid="top-timeline-now-indicator"
      className="pointer-events-none absolute top-0 bottom-0 z-40 w-0"
      style={{ left }}
    >
      <div className="absolute top-0 bottom-1 left-0 w-px -translate-x-1/2 bg-red-500/90 shadow-[0_0_0_1px_rgba(255,255,255,0.85)]" />
      <div className="absolute top-0 left-0 size-1.5 -translate-x-1/2 rounded-full bg-red-500 shadow-[0_0_0_1px_rgba(255,255,255,0.95)]" />
      <div className="absolute top-1/2 left-0 -translate-x-1/2 -translate-y-1/2 rounded-full bg-red-500 px-2 py-1 font-mono text-[10px] leading-none font-semibold whitespace-nowrap text-white opacity-0 shadow-xs transition-opacity group-hover/timeline-strip:opacity-100">
        {label}
      </div>
    </div>
  );
}

const SessionTimelineBar = memo(
  ({ item, timezone }: { item: MeetingTimelineEntry; timezone?: string }) => {
    const openNew = useTabs((state) => state.openNew);
    const deleteSession = useDeleteSession();
    const sessionRow = main.UI.useRow("sessions", item.id, main.STORE_ID) as
      | TimelineSessionRow
      | undefined;
    const storeTitle = main.UI.useCell(
      "sessions",
      item.id,
      "title",
      main.STORE_ID,
    ) as string | undefined;
    const title = useSessionTitle(item.id, storeTitle);
    const { sessionMode, stop, amplitude } = useListener((state) => ({
      sessionMode: state.getSessionMode(item.id),
      stop: state.stop,
      amplitude: state.live.amplitude,
    }));
    const isEnhancing = useIsSessionEnhancing(item.id);
    const isLive = sessionMode === "active";
    const showSpinner =
      sessionMode === "finalizing" ||
      sessionMode === "running_batch" ||
      isEnhancing;
    const sessionEvent = useMemo(
      () => (sessionRow ? getSessionEvent(sessionRow) : null),
      [sessionRow?.event_json],
    );

    const openSession = useCallback(() => {
      openNew({ id: item.id, type: "sessions" });
    }, [item.id, openNew]);

    const handleDragStart = useCallback(
      (event: DragEvent<HTMLButtonElement>) => {
        writeSessionContextDragData(
          event.dataTransfer,
          item.id,
          title || item.title || "Untitled",
        );
      },
      [item.id, item.title, title],
    );

    const handleDelete = useCallback(() => {
      deleteSession(item.id, sessionEvent?.tracking_id);
    }, [deleteSession, item.id, sessionEvent]);

    const handleShowInFinder = useCallback(async () => {
      const result = await fsSyncCommands.sessionDir(item.id);
      if (result.status === "ok") {
        await openerCommands.openPath(result.data, null);
      }
    }, [item.id]);

    const contextMenu = useMemo<MenuItemDef[]>(
      () => [
        {
          id: "open-new-tab",
          text: "Open in New Tab",
          action: openSession,
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
      [openSession, handleShowInFinder, handleDelete],
    );

    return (
      <TimelineCarouselCard item={item}>
        <SessionPreviewCard sessionId={item.id} side="bottom" enabled>
          <TimelineCardButton
            item={item}
            title={title || item.title || "Untitled"}
            timezone={timezone}
            isLive={isLive}
            amplitude={Math.max(
              0.25,
              Math.min(Math.hypot(amplitude.mic, amplitude.speaker), 1),
            )}
            showSpinner={showSpinner}
            onClick={openSession}
            onDragStart={handleDragStart}
            onStop={stop}
            contextMenu={contextMenu}
            draggable
          />
        </SessionPreviewCard>
      </TimelineCarouselCard>
    );
  },
);

const EventTimelineBar = memo(
  ({ item, timezone }: { item: MeetingTimelineEntry; timezone?: string }) => {
    const store = main.UI.useStore(main.STORE_ID);
    const openNew = useTabs((state) => state.openNew);
    const { ignoreEvent, ignoreSeries } = useIgnoredEvents();

    const openEvent = useCallback(() => {
      if (!store) {
        return;
      }

      const sessionId = getOrCreateSessionForEventId(
        store,
        item.id,
        item.title,
      );
      const tab = { id: sessionId, type: "sessions" } as const;

      openNew(tab);
    }, [item.id, item.title, openNew, store]);

    const handleIgnore = useCallback(() => {
      if (!item.trackingId) {
        return;
      }

      ignoreEvent(item.trackingId);
    }, [item.trackingId, ignoreEvent]);

    const handleIgnoreSeries = useCallback(() => {
      if (!item.recurrenceSeriesId) {
        return;
      }

      ignoreSeries(item.recurrenceSeriesId);
    }, [item.recurrenceSeriesId, ignoreSeries]);

    const contextMenu = useMemo<MenuItemDef[]>(() => {
      const menu: MenuItemDef[] = [
        {
          id: "open-new-tab",
          text: "Open in New Tab",
          action: openEvent,
        },
        { separator: true },
        {
          id: "ignore",
          text: item.recurrenceSeriesId ? "Delete This Event" : "Delete Event",
          action: handleIgnore,
        },
      ];

      if (item.recurrenceSeriesId) {
        menu.push({
          id: "ignore-series",
          text: "Delete All Recurring Events",
          action: handleIgnoreSeries,
        });
      }

      return menu;
    }, [openEvent, handleIgnore, handleIgnoreSeries, item.recurrenceSeriesId]);

    return (
      <TimelineCarouselCard item={item}>
        <TimelineCardButton
          item={item}
          title={item.title || "Untitled"}
          timezone={timezone}
          onClick={openEvent}
          contextMenu={contextMenu}
        />
      </TimelineCarouselCard>
    );
  },
);

type TimelineRenderItem = {
  kind: "item";
  id: string;
  item: MeetingTimelineEntry;
  start: Date;
};

type TimelineCreateNoteItem = {
  kind: "create-note";
  id: string;
  start: Date;
};

type TimelineOpenCalendarItem = {
  kind: "open-calendar";
  id: string;
  start: Date;
};

type TimelineCarouselItem =
  | TimelineRenderItem
  | TimelineCreateNoteItem
  | TimelineOpenCalendarItem;

function TimelineCarouselCard({
  item,
  children,
}: {
  item: MeetingTimelineEntry;
  children: ReactNode;
}) {
  return (
    <div
      data-timeline-start-ms={item.start.getTime()}
      className={cn([
        "group/timeline-card relative shrink-0 snap-start",
        "origin-top transition-transform focus-within:z-30 focus-within:scale-[1.02] hover:z-30 hover:scale-[1.02]",
        item.selected && "z-20",
      ])}
      style={{ width: TIMELINE_CAROUSEL_CARD_WIDTH }}
    >
      {children}
    </div>
  );
}

function TimelineCreateNoteCard({
  item,
  onClick,
}: {
  item: TimelineCreateNoteItem;
  onClick: () => void;
}) {
  return (
    <div
      data-timeline-start-ms={item.start.getTime()}
      className="relative shrink-0 snap-start"
      style={{ width: TIMELINE_CAROUSEL_CARD_WIDTH }}
    >
      <button
        type="button"
        className={cn([
          "flex h-10 w-full flex-col justify-center rounded-md border border-dashed border-neutral-300 bg-white/80 px-2 text-left shadow-xs",
          "transition-colors hover:border-neutral-600 hover:bg-white focus-visible:ring-2 focus-visible:ring-neutral-900 focus-visible:outline-hidden",
        ])}
        onClick={onClick}
      >
        <span className="flex min-w-0 items-center gap-1.5 truncate text-xs font-semibold text-neutral-700">
          <PlusIcon size={12} className="shrink-0" />
          <span className="truncate">Create new note</span>
        </span>
      </button>
    </div>
  );
}

function TimelineOpenCalendarCard({
  item,
  onClick,
}: {
  item: TimelineOpenCalendarItem;
  onClick: () => void;
}) {
  return (
    <div
      data-timeline-start-ms={item.start.getTime()}
      className="relative shrink-0 snap-start"
      style={{ width: TIMELINE_CAROUSEL_CARD_WIDTH }}
    >
      <button
        type="button"
        className={cn([
          "flex h-10 w-full items-center gap-1.5 rounded-md border border-dashed border-neutral-300 bg-white/80 px-2 text-left text-xs font-semibold text-neutral-700 shadow-xs",
          "transition-colors hover:border-neutral-600 hover:bg-white focus-visible:ring-2 focus-visible:ring-neutral-900 focus-visible:outline-hidden",
        ])}
        onClick={onClick}
      >
        <CalendarIcon size={12} className="shrink-0" />
        <span className="truncate">See more</span>
      </button>
    </div>
  );
}

function FadedTimelineLabel({
  children,
  className,
  style,
}: {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
}) {
  const fadeMask =
    "linear-gradient(to right, black calc(100% - 20px), transparent)";

  return (
    <span
      className={cn(["overflow-hidden whitespace-nowrap", className])}
      style={{
        ...style,
        WebkitMaskImage: fadeMask,
        maskImage: fadeMask,
      }}
    >
      {children}
    </span>
  );
}

function TimelineCardButton({
  item,
  title,
  timezone,
  isLive,
  amplitude,
  showSpinner,
  onClick,
  onDragStart,
  onStop,
  contextMenu,
  draggable,
}: {
  item: MeetingTimelineEntry;
  title: string;
  timezone?: string;
  isLive?: boolean;
  amplitude?: number;
  showSpinner?: boolean;
  onClick: (event: MouseEvent<HTMLButtonElement>) => void;
  onDragStart?: (event: DragEvent<HTMLButtonElement>) => void;
  onStop?: () => void;
  contextMenu?: MenuItemDef[];
  draggable?: boolean;
}) {
  const showContextMenu = useNativeContextMenu(contextMenu ?? []);
  const handleContextMenu = useCallback<MouseEventHandler<HTMLButtonElement>>(
    (event) => {
      if (!contextMenu) {
        event.preventDefault();
        event.stopPropagation();
        return;
      }

      void showContextMenu(event);
    },
    [contextMenu, showContextMenu],
  );
  const handleStopClick = useCallback<MouseEventHandler<HTMLButtonElement>>(
    (event) => {
      event.preventDefault();
      event.stopPropagation();
      onStop?.();
    },
    [onStop],
  );
  const handlePointerDown = useCallback(
    (event: PointerEvent<HTMLButtonElement>) => {
      if (!draggable) {
        return;
      }

      event.stopPropagation();
    },
    [draggable],
  );
  const startLabel = formatTimelineStartLabel(item.start, timezone);
  const showLiveStop = item.type === "session" && isLive && onStop;
  const showSuffixSpinner =
    item.type === "session" && !!showSpinner && !showLiveStop;
  const showSuffix = showLiveStop || showSuffixSpinner;

  return (
    <div className="group/live-timeline-card relative h-10 w-full">
      <button
        type="button"
        onClick={onClick}
        onDragStart={onDragStart}
        onContextMenu={handleContextMenu}
        onPointerDown={handlePointerDown}
        draggable={draggable}
        className={cn([
          "flex h-10 w-full flex-col justify-center rounded-md border py-0 pl-2 text-left shadow-xs",
          showSuffix ? "pr-8" : "pr-2",
          "transition-colors hover:border-neutral-700 focus-visible:ring-2 focus-visible:ring-neutral-900 focus-visible:outline-hidden",
          item.type === "session" &&
            (showLiveStop
              ? "border-red-500 bg-red-500 text-white hover:border-red-600 hover:bg-red-600"
              : item.selected
                ? "border-neutral-900 bg-neutral-900 text-white hover:bg-neutral-800"
                : "border-neutral-300 bg-white text-neutral-800 hover:bg-neutral-50"),
          item.type === "event" &&
            "border-dashed border-neutral-300 bg-white/80 text-neutral-600 hover:bg-white",
          item.muted && !item.selected && !showLiveStop && "opacity-60",
        ])}
      >
        <span className="flex min-w-0 items-center gap-1.5">
          <FadedTimelineLabel className="min-w-0 flex-1 text-xs font-semibold">
            {title}
          </FadedTimelineLabel>
        </span>
        <FadedTimelineLabel
          className={cn([
            "font-mono text-[10px]",
            item.selected || showLiveStop
              ? "text-white/65"
              : "text-neutral-500",
          ])}
        >
          {startLabel}
        </FadedTimelineLabel>
      </button>
      {showSuffixSpinner ? (
        <span
          role="status"
          aria-label="Loading timeline item"
          className={cn([
            "absolute top-1/2 right-2 flex size-5 -translate-y-1/2 items-center justify-center",
            item.selected ? "text-white/70" : "text-neutral-500",
          ])}
        >
          <Spinner size={12} />
        </span>
      ) : showLiveStop ? (
        <button
          type="button"
          aria-label="Stop listening"
          onClick={handleStopClick}
          className={cn([
            "absolute top-1/2 right-2 flex size-5 -translate-y-1/2 items-center justify-center rounded-sm",
            "text-white/80 transition-colors hover:bg-white/15 hover:text-white",
            "focus-visible:ring-2 focus-visible:ring-white/70 focus-visible:outline-hidden",
          ])}
        >
          <span
            aria-hidden
            className="flex items-center justify-center group-hover/live-timeline-card:hidden"
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
            className="hidden items-center justify-center group-hover/live-timeline-card:flex"
          >
            <SquareIcon size={10} className="fill-current" />
          </span>
        </button>
      ) : null}
    </div>
  );
}

function buildTimelineRenderItems(
  items: MeetingTimelineEntry[],
  range: {
    startInclusive: Date;
    endExclusive: Date;
  },
): TimelineRenderItem[] {
  const startInclusiveMs = range.startInclusive.getTime();
  const endExclusiveMs = range.endExclusive.getTime();

  return [...items]
    .filter((item) => {
      if (item.selected) {
        return true;
      }

      const startMs = item.start.getTime();
      const endMs = normalizeEndMs(item.start, item.end);

      return startMs < endExclusiveMs && endMs >= startInclusiveMs;
    })
    .sort((a, b) => {
      const startDiff = a.start.getTime() - b.start.getTime();
      if (startDiff !== 0) {
        return startDiff;
      }

      return normalizeEndMs(a.start, a.end) - normalizeEndMs(b.start, b.end);
    })
    .map((item) => ({
      kind: "item" as const,
      id: `${item.type}-${item.id}`,
      item,
      start: item.start,
    }));
}

function buildTimelineCarouselItems({
  renderItems,
  currentDate,
  startInclusive,
  endExclusive,
  hasHiddenPastItems,
  hasHiddenFutureItems,
  currentTime,
  includeCreateNote,
}: {
  renderItems: TimelineRenderItem[];
  currentDate: Date;
  startInclusive: Date;
  endExclusive: Date;
  hasHiddenPastItems: boolean;
  hasHiddenFutureItems: boolean;
  currentTime: Date;
  includeCreateNote: boolean;
}): TimelineCarouselItem[] {
  const items: TimelineRenderItem[] = [...renderItems];

  const sortedItems = items.sort(
    (a, b) =>
      getTimelineCarouselItemStart(a).getTime() -
      getTimelineCarouselItemStart(b).getTime(),
  );
  const visibleItems = includeCreateNote
    ? insertCreateNoteAtCurrentTime(sortedItems, {
        currentDate,
        currentTime,
      })
    : sortedItems;

  const result: TimelineCarouselItem[] = hasHiddenPastItems
    ? [
        {
          kind: "open-calendar",
          id: `open-calendar-${startInclusive.getTime()}`,
          start: startInclusive,
        },
        ...visibleItems,
      ]
    : visibleItems;

  if (hasHiddenFutureItems) {
    result.push({
      kind: "open-calendar",
      id: `open-calendar-${endExclusive.getTime()}`,
      start: endExclusive,
    });
  }

  return result;
}

function insertCreateNoteAtCurrentTime(
  items: TimelineRenderItem[],
  {
    currentDate,
    currentTime,
  }: {
    currentDate: Date;
    currentTime: Date;
  },
): TimelineCarouselItem[] {
  const createNote: TimelineCreateNoteItem = {
    kind: "create-note",
    id: `create-note-${currentDate.getTime()}`,
    start: currentTime,
  };
  const chronologicalIndex = items.findIndex(
    (item) => item.start.getTime() > currentTime.getTime(),
  );

  if (chronologicalIndex === -1) {
    return [...items, createNote];
  }

  return [
    ...items.slice(0, chronologicalIndex),
    createNote,
    ...items.slice(chronologicalIndex),
  ];
}

function hasTimelineEntriesBefore(
  entries: MeetingTimelineEntry[],
  startInclusive: Date,
): boolean {
  const startInclusiveMs = startInclusive.getTime();

  return entries.some(
    (entry) =>
      !entry.selected &&
      normalizeEndMs(entry.start, entry.end) < startInclusiveMs,
  );
}

function hasTimelineEntriesAfter(
  entries: MeetingTimelineEntry[],
  endExclusive: Date,
): boolean {
  const endExclusiveMs = endExclusive.getTime();

  return entries.some(
    (entry) => !entry.selected && entry.start.getTime() >= endExclusiveMs,
  );
}

function getTimelineCarouselWidth(items: TimelineCarouselItem[]): number {
  if (items.length === 0) {
    return 1;
  }

  const contentWidth = items.reduce(
    (sum, item) => sum + getTimelineCarouselItemWidth(item),
    0,
  );

  return (
    TIMELINE_CAROUSEL_PADDING * 2 +
    TIMELINE_CAROUSEL_END_PADDING +
    contentWidth +
    TIMELINE_CAROUSEL_GAP * Math.max(0, items.length - 1)
  );
}

export function isTimelineWindowDrag(
  start: { clientX: number; clientY: number },
  current: { clientX: number; clientY: number },
): boolean {
  const deltaX = current.clientX - start.clientX;
  const deltaY = current.clientY - start.clientY;

  return (
    deltaX * deltaX + deltaY * deltaY >=
    TIMELINE_WINDOW_DRAG_THRESHOLD_PX * TIMELINE_WINDOW_DRAG_THRESHOLD_PX
  );
}

function getTimelineCarouselX(
  items: TimelineCarouselItem[],
  timestampMs: number,
): number {
  if (items.length === 0) {
    return 0;
  }

  let left = TIMELINE_CAROUSEL_PADDING;
  let closestLeft = left;
  let closestDistance = Number.POSITIVE_INFINITY;

  for (const item of items) {
    const distance = Math.abs(
      getTimelineCarouselItemStart(item).getTime() - timestampMs,
    );

    if (distance < closestDistance) {
      closestDistance = distance;
      closestLeft = left;
    }

    left += getTimelineCarouselItemWidth(item) + TIMELINE_CAROUSEL_GAP;
  }

  return closestLeft;
}

function getTimelineCarouselDateX(
  items: TimelineCarouselItem[],
  date: Date,
  timezone?: string,
): number {
  const range = getTimelineCarouselDateRange(items, date, timezone);

  if (range) {
    return range.left;
  }

  return getTimelineCarouselX(items, date.getTime());
}

function getTimelineCarouselNowX(
  items: TimelineCarouselItem[],
  current: Date,
  timezone?: string,
): number | null {
  const currentMs = current.getTime();
  const positionedItems = getPositionedTimelineCarouselItems(items);

  for (const positioned of positionedItems) {
    if (positioned.item.kind !== "item") {
      continue;
    }

    const startMs = positioned.item.item.start.getTime();
    const end = positioned.item.item.end;
    const endMs = end?.getTime();

    if (endMs && startMs <= currentMs && currentMs <= endMs) {
      if (endMs <= startMs) {
        return positioned.right;
      }

      const progress = (currentMs - startMs) / (endMs - startMs);
      return (
        positioned.left + positioned.width * Math.min(Math.max(progress, 0), 1)
      );
    }
  }

  const todayItems = positionedItems.filter((positioned) =>
    isSameTimelineDay(
      getTimelineCarouselItemStart(positioned.item),
      current,
      timezone,
    ),
  );

  if (todayItems.length === 0) {
    return null;
  }

  const nextItem = todayItems.find(
    (positioned) =>
      getTimelineCarouselItemStart(positioned.item).getTime() > currentMs,
  );
  let previousItem: (typeof todayItems)[number] | undefined;
  for (const positioned of todayItems) {
    if (getTimelineCarouselItemStart(positioned.item).getTime() <= currentMs) {
      previousItem = positioned;
    }
  }

  if (previousItem && nextItem) {
    if (previousItem.item.kind === "create-note") {
      return previousItem.left;
    }

    if (nextItem.item.kind === "create-note") {
      return nextItem.left;
    }

    return previousItem.right + (nextItem.left - previousItem.right) / 2;
  }

  if (nextItem) {
    return nextItem.left;
  }

  if (!previousItem) {
    return null;
  }

  if (previousItem.item.kind === "create-note") {
    return previousItem.left;
  }

  if (previousItem.item.kind !== "item") {
    return previousItem.left + previousItem.width / 2;
  }

  return previousItem.right;
}

function getTimelineCarouselDateDirection({
  items,
  date,
  timezone,
  scrollLeft,
  viewportWidth,
}: {
  items: TimelineCarouselItem[];
  date: Date;
  timezone?: string;
  scrollLeft: number;
  viewportWidth: number;
}): TodayChipDirection | null {
  const range = getTimelineCarouselDateRange(items, date, timezone);

  if (!range) {
    return null;
  }

  const viewportLeft = scrollLeft;
  const viewportRight = scrollLeft + viewportWidth;

  if (range.right <= viewportLeft) {
    return "left";
  }

  if (range.left >= viewportRight) {
    return "right";
  }

  return null;
}

export function getTimelineCarouselNowDirection({
  nowX,
  scrollLeft,
  viewportWidth,
}: {
  nowX: number | null;
  scrollLeft: number;
  viewportWidth: number;
}): TodayChipDirection | null {
  if (nowX === null) {
    return null;
  }

  const viewportRight = scrollLeft + viewportWidth;

  if (nowX < scrollLeft) {
    return "left";
  }

  if (nowX > viewportRight) {
    return "right";
  }

  return null;
}

function getTimelineCarouselDateRange(
  items: TimelineCarouselItem[],
  date: Date,
  timezone?: string,
): { left: number; right: number } | null {
  let left = TIMELINE_CAROUSEL_PADDING;
  let range: { left: number; right: number } | null = null;

  for (const item of items) {
    const width = getTimelineCarouselItemWidth(item);
    const right = left + width;

    if (isSameTimelineDay(getTimelineCarouselItemStart(item), date, timezone)) {
      range = range
        ? {
            left: Math.min(range.left, left),
            right: Math.max(range.right, right),
          }
        : { left, right };
    }

    left = right + TIMELINE_CAROUSEL_GAP;
  }

  return range;
}

function getPositionedTimelineCarouselItems(
  items: TimelineCarouselItem[],
): Array<{
  item: TimelineCarouselItem;
  left: number;
  right: number;
  width: number;
}> {
  let left = TIMELINE_CAROUSEL_PADDING;

  return items.map((item) => {
    const width = getTimelineCarouselItemWidth(item);
    const right = left + width;
    const positionedItem = { item, left, right, width };
    left = right + TIMELINE_CAROUSEL_GAP;
    return positionedItem;
  });
}

function getTimelineCarouselAnchorKey(
  items: TimelineCarouselItem[],
  selectedSessionId: string | null,
): string {
  const first = items[0];
  const last = items[items.length - 1];

  return [
    selectedSessionId ?? "today",
    items.length,
    first ? getTimelineCarouselAnchorToken(first) : 0,
    last ? getTimelineCarouselAnchorToken(last) : 0,
  ].join(":");
}

function getTimelineCarouselAnchorToken(item: TimelineCarouselItem): string {
  return item.kind === "create-note"
    ? item.id
    : String(getTimelineCarouselItemStart(item).getTime());
}

function getTimelineCarouselItemStart(item: TimelineCarouselItem): Date {
  return item.start;
}

function getTimelineCarouselItemWidth(_item: TimelineCarouselItem): number {
  return TIMELINE_CAROUSEL_CARD_WIDTH;
}

function getTimelineDayStart(date: Date, timezone?: string): Date {
  return startOfDay(timezone ? new TZDate(date, timezone) : date);
}

function isSameTimelineDay(
  first: Date,
  second: Date,
  timezone?: string,
): boolean {
  const firstDate = timezone ? new TZDate(first, timezone) : first;
  const secondDate = timezone ? new TZDate(second, timezone) : second;

  return format(firstDate, "yyyy-MM-dd") === format(secondDate, "yyyy-MM-dd");
}

function buildMeetingTimelineEntries({
  timelineEventsTable,
  timelineSessionsTable,
  sessionRecordingRanges,
  selectedSessionId,
  liveSessionId,
  now,
  isIgnored,
}: {
  timelineEventsTable: TimelineEventsTable;
  timelineSessionsTable: TimelineSessionsTable;
  sessionRecordingRanges: ReadonlyMap<string, SessionRecordingRange>;
  selectedSessionId: string | null;
  liveSessionId: string | null;
  now: number;
  isIgnored: (
    trackingId?: string | null,
    recurrenceSeriesId?: string | null,
  ) => boolean;
}): MeetingTimelineEntry[] {
  const entries: MeetingTimelineEntry[] = [];
  const sessionTrackingIds = new Set<string>();

  if (timelineSessionsTable) {
    Object.entries(timelineSessionsTable).forEach(([sessionId, row]) => {
      const entry = getSessionTimelineEntry({
        sessionId,
        row,
        recordingRange: sessionRecordingRanges.get(sessionId),
        selected: selectedSessionId === sessionId,
        isLive: liveSessionId === sessionId,
        now,
      });

      if (!entry) {
        return;
      }

      entries.push(entry);

      const event = getSessionEvent(row);
      if (event?.tracking_id) {
        sessionTrackingIds.add(event.tracking_id);
      }
    });
  }

  if (timelineEventsTable) {
    Object.entries(timelineEventsTable).forEach(([eventId, row]) => {
      if (row.is_all_day) {
        return;
      }

      if (
        row.tracking_id_event &&
        sessionTrackingIds.has(row.tracking_id_event)
      ) {
        return;
      }

      if (isIgnored(row.tracking_id_event, row.recurrence_series_id)) {
        return;
      }

      const entry = getEventTimelineEntry({ eventId, row, now });
      if (entry) {
        entries.push(entry);
      }
    });
  }

  return entries;
}

function getSessionTimelineEntry({
  sessionId,
  row,
  recordingRange,
  selected,
  isLive,
  now,
}: {
  sessionId: string;
  row: TimelineSessionRow;
  recordingRange?: SessionRecordingRange;
  selected: boolean;
  isLive: boolean;
  now: number;
}): MeetingTimelineEntry | null {
  const event = getSessionEvent(row);
  const start =
    recordingRange?.start ?? safeParseDate(event?.started_at ?? row.created_at);

  if (!start) {
    return null;
  }

  const eventEnd = safeParseDate(event?.ended_at);
  const recordedOrScheduledEnd = recordingRange?.end ?? eventEnd ?? null;
  const end = isLive && !eventEnd ? new Date(now) : recordedOrScheduledEnd;

  return {
    id: sessionId,
    type: "session",
    title: row.title || event?.title || "Untitled",
    calendarId: event?.calendar_id ?? null,
    start,
    end,
    selected,
    muted: start.getTime() > now,
  };
}

function getEventTimelineEntry({
  eventId,
  row,
  now,
}: {
  eventId: string;
  row: TimelineEventRow;
  now: number;
}): MeetingTimelineEntry | null {
  const parsedStart = safeParseDate(row.started_at);
  const parsedEnd = safeParseDate(row.ended_at);
  const start =
    parsedStart ??
    (parsedEnd ? new Date(parsedEnd.getTime() - TIMELINE_BLOCK_MS) : null);

  if (!start) {
    return null;
  }

  const endMs = normalizeEndMs(start, parsedEnd);
  if (endMs < now) {
    return null;
  }

  return {
    id: eventId,
    type: "event",
    title: row.title || "Untitled",
    calendarId: row.calendar_id ?? null,
    trackingId: row.tracking_id_event ?? null,
    recurrenceSeriesId: row.recurrence_series_id ?? null,
    start,
    end: parsedEnd,
    selected: false,
    muted: start.getTime() > now,
  };
}

export function formatTimelineStartLabel(
  date: Date,
  timezone?: string,
): string {
  const displayDate = timezone ? new TZDate(date, timezone) : date;
  return `${formatRelativeTimelineDay(date, timezone)} ${format(displayDate, "h:mm a")}`;
}

function formatRelativeTimelineDay(date: Date, timezone?: string): string {
  const displayDate = timezone ? new TZDate(date, timezone) : date;
  const displayNow = timezone ? new TZDate(new Date(), timezone) : new Date();
  const daysDiff = differenceInCalendarDays(displayDate, displayNow);
  const absDays = Math.abs(daysDiff);

  if (daysDiff === 0) {
    return "Today";
  }

  if (daysDiff === -1) {
    return "Yesterday";
  }

  if (daysDiff === 1) {
    return "Tomorrow";
  }

  if (daysDiff < 0 && absDays <= 6) {
    return `${absDays} days ago`;
  }

  if (daysDiff > 0 && absDays <= 6) {
    return `In ${absDays} days`;
  }

  return format(displayDate, "M/d");
}
