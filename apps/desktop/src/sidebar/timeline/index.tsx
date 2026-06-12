import {
  ArrowDownIcon,
  ArrowUpIcon,
  CalendarDaysIcon,
  SunIcon,
} from "lucide-react";
import {
  type ReactNode,
  type RefCallback,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  useManager,
  useRunningTaskRunIds,
  useScheduledTaskRunIds,
} from "tinytick/ui-react";

import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import { useAnchor, useAutoScrollToAnchor } from "./anchor";
import { TimelineItemComponent } from "./item";
import {
  CurrentTimeIndicator,
  useCurrentTimeMs,
  useSmartCurrentTime,
} from "./realtime";
import {
  buildTimelineBuckets,
  calculateTodayIndicatorPlacement,
  deriveTimelineWindowData,
  getItemTimestamp,
  type TimelineBucket,
  type TimelineEventsTable,
  type TimelineIndicatorPlacement,
  type TimelineItem,
  type TimelinePrecision,
  type TimelineSessionsTable,
} from "./utils";

import { CALENDAR_SYNC_TASK_ID } from "~/services/calendar";
import { useConfigValue } from "~/shared/config";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { useIgnoredEvents } from "~/store/tinybase/hooks";
import {
  captureSessionData,
  deleteSessionCascade,
  finalizeSessionDeletion,
} from "~/store/tinybase/store/deleteSession";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";
import { useTimelineSelection } from "~/store/zustand/timeline-selection";
import { useUndoDelete } from "~/store/zustand/undo-delete";
import { useListener } from "~/stt/contexts";

const DUE_CALENDAR_SYNC_VISIBLE_WINDOW_MS = 1000;
const CALENDAR_SYNC_STATUS_TICK_MS = 500;
type SidebarCalendarSyncStatus = "idle" | "scheduled" | "syncing";

export function TimelineView({
  showOpenCalendarButton = true,
  topChromeInset = false,
}: {
  showOpenCalendarButton?: boolean;
  topChromeInset?: boolean;
} = {}) {
  const timezone = useConfigValue("timezone") || undefined;
  const { timelineEventsTable, timelineSessionsTable } = useTimelineTables();
  const [showIgnored, setShowIgnored] = useState(false);
  const [isScrolledToTop, setIsScrolledToTop] = useState(true);
  const [isScrolledToBottom, setIsScrolledToBottom] = useState(true);

  const { isIgnored } = useIgnoredEvents();
  const { buckets, hasMoreFutureItems, hasVisibleCalendarEvents } =
    useTimelineData({
      isEventIgnored: isIgnored,
      showIgnored,
      timelineEventsTable,
      timelineSessionsTable,
      timezone,
    });
  const openNew = useTabs((state) => state.openNew);
  const calendarSyncStatus = useCalendarSyncStatus();
  const showCalendarSyncStatus =
    calendarSyncStatus !== "idle" && !hasVisibleCalendarEvents;

  const showOpenCalendarChip =
    showOpenCalendarButton && isScrolledToTop && hasMoreFutureItems;

  const hasToday = useMemo(
    () => buckets.some((bucket) => bucket.label === "Today"),
    [buckets],
  );
  const currentTimeMs = useCurrentTimeMs();
  const activeSessionId = useListener((state) =>
    state.live.status === "active" || state.live.status === "finalizing"
      ? state.live.sessionId
      : null,
  );
  const hasActiveVisibleSession = useMemo(
    () =>
      !!activeSessionId &&
      buckets.some((bucket) =>
        bucket.items.some(
          (item) => item.type === "session" && item.id === activeSessionId,
        ),
      ),
    [activeSessionId, buckets],
  );

  const currentTab = useTabs((state) => state.currentTab);

  const selectedSessionId = useMemo(() => {
    return currentTab?.type === "sessions" ? currentTab.id : undefined;
  }, [currentTab]);

  const store = main.UI.useStore(main.STORE_ID);

  const selectedIds = useTimelineSelection((s) => s.selectedIds);
  const clearSelection = useTimelineSelection((s) => s.clear);
  const indexes = main.UI.useIndexes(main.STORE_ID);
  const invalidateResource = useTabs((state) => state.invalidateResource);
  const addDeletion = useUndoDelete((state) => state.addDeletion);

  const flatItemKeys = useMemo(() => {
    const keys: string[] = [];
    for (const bucket of buckets) {
      for (const item of bucket.items) {
        keys.push(`${item.type}-${item.id}`);
      }
    }
    return keys;
  }, [buckets]);

  const {
    containerRef,
    isAnchorVisible: isTodayVisible,
    isScrolledPastAnchor: isScrolledPastToday,
    scrollToAnchor: scrollToToday,
    registerAnchor: setCurrentTimeIndicatorRef,
    anchorNode: todayAnchorNode,
  } = useAnchor();
  const showTopNowChip = !isTodayVisible && isScrolledPastToday;
  const topChromeChipCount = [
    showOpenCalendarChip,
    topChromeInset && showCalendarSyncStatus,
    topChromeInset && showTopNowChip,
  ].filter(Boolean).length;
  const topSpacerClassName = topChromeInset
    ? topChromeChipCount > 1
      ? "h-28"
      : topChromeChipCount === 1
        ? "h-20"
        : "h-12"
    : "h-10";
  const bucketHeaderTopClassName = topChromeInset
    ? topChromeChipCount > 1
      ? "top-28"
      : topChromeChipCount === 1
        ? "top-20"
        : "top-12"
    : "top-0";
  const selectedSessionScrollFrameRef = useRef<number | null>(null);
  const scrollSelectedSessionIntoView = useCallback<
    RefCallback<HTMLDivElement>
  >(
    (node) => {
      if (selectedSessionScrollFrameRef.current !== null) {
        cancelAnimationFrame(selectedSessionScrollFrameRef.current);
        selectedSessionScrollFrameRef.current = null;
      }

      if (!node || currentTab?.type !== "sessions") {
        return;
      }

      selectedSessionScrollFrameRef.current = requestAnimationFrame(() => {
        selectedSessionScrollFrameRef.current = null;
        scrollTimelineItemIntoView(containerRef.current, node);
      });
    },
    [containerRef, currentTab],
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    const updateScrollPosition = () => {
      const maxScrollTop = Math.max(
        0,
        container.scrollHeight - container.clientHeight,
      );
      const nextScrollTop = container.scrollTop;
      const scrolledToTop = nextScrollTop <= 12;

      setIsScrolledToTop(scrolledToTop);
      setIsScrolledToBottom(maxScrollTop - nextScrollTop <= 12);
    };

    updateScrollPosition();
    container.addEventListener("scroll", updateScrollPosition, {
      passive: true,
    });

    return () => {
      container.removeEventListener("scroll", updateScrollPosition);
    };
  }, [containerRef, buckets.length, flatItemKeys.length]);

  const scrollFadeMask = useMemo(() => {
    const topFadeEnd = isScrolledToTop ? "0px" : "28px";
    const bottomFadeStart = isScrolledToBottom ? "100%" : "calc(100% - 28px)";

    return `linear-gradient(to bottom, transparent 0, #000 ${topFadeEnd}, #000 ${bottomFadeStart}, transparent 100%)`;
  }, [isScrolledToTop, isScrolledToBottom]);

  const todayBucketLength = useMemo(() => {
    const b = buckets.find((bucket) => bucket.label === "Today");
    return b?.items.length ?? 0;
  }, [buckets]);

  useAutoScrollToAnchor({
    scrollFn: scrollToToday,
    isVisible: isTodayVisible,
    anchorNode: todayAnchorNode,
    deps: [todayBucketLength],
  });

  const indicatorIndex = useMemo(() => {
    if (hasToday) {
      return -1;
    }
    return getFallbackIndicatorIndex(buckets, Date.now());
  }, [buckets, hasToday, currentTimeMs]);

  const toggleShowIgnored = useCallback(() => {
    setShowIgnored((prev) => !prev);
  }, []);

  const handleOpenCalendar = useCallback(() => {
    openNew({ type: "calendar" });
  }, [openNew]);

  const handleDeleteSelected = useCallback(() => {
    if (!store || !indexes) {
      return;
    }

    const sessionIds = selectedIds
      .filter((key) => key.startsWith("session-"))
      .map((key) => key.replace("session-", ""));

    const batchId = sessionIds.length > 1 ? crypto.randomUUID() : undefined;

    for (const sessionId of sessionIds) {
      const capturedData = captureSessionData(store, indexes, sessionId);

      invalidateResource("sessions", sessionId);
      void deleteSessionCascade(store, indexes, sessionId, {
        deferFilesystemDelete: true,
      });

      if (capturedData) {
        addDeletion(
          capturedData,
          () => {
            void finalizeSessionDeletion(sessionId);
          },
          batchId,
        );
      }
    }

    clearSelection();
  }, [
    store,
    indexes,
    selectedIds,
    invalidateResource,
    addDeletion,
    clearSelection,
  ]);

  const sessionCount = useMemo(
    () => selectedIds.filter((key) => key.startsWith("session-")).length,
    [selectedIds],
  );

  const contextMenuItems = useMemo(
    () =>
      selectedIds.length > 0
        ? [
            {
              id: "delete-selected",
              text: `Delete Selected (${sessionCount})`,
              action: handleDeleteSelected,
              disabled: sessionCount === 0,
            },
          ]
        : [
            {
              id: "toggle-ignored",
              text: showIgnored ? "Hide Deleted Events" : "Show Deleted Events",
              action: toggleShowIgnored,
            },
          ],
    [
      selectedIds,
      sessionCount,
      handleDeleteSelected,
      showIgnored,
      toggleShowIgnored,
    ],
  );

  const showContextMenu = useNativeContextMenu(contextMenuItems);

  return (
    <div className="relative h-full">
      <div
        ref={containerRef}
        data-sidebar-timeline-scroll
        onContextMenu={showContextMenu}
        className={cn([
          "scrollbar-hide flex h-full flex-col overflow-y-auto",
          "rounded-xl",
        ])}
        style={{
          WebkitMaskImage: scrollFadeMask,
          maskImage: scrollFadeMask,
        }}
      >
        {(topChromeInset || hasMoreFutureItems) && (
          <div
            aria-hidden
            data-sidebar-timeline-top-spacer
            className={cn([topSpacerClassName, "shrink-0"])}
          />
        )}
        {buckets.map((bucket, index) => {
          const isToday = bucket.label === "Today";
          const shouldPlaceIndicatorBefore =
            !hasToday && indicatorIndex === index;
          const shouldRenderIndicatorBefore =
            shouldPlaceIndicatorBefore && !hasActiveVisibleSession;
          const shouldRenderIndicatorAnchorBefore =
            shouldPlaceIndicatorBefore && hasActiveVisibleSession;
          const isTopIndicator = shouldRenderIndicatorBefore && index === 0;

          return (
            <div key={bucket.label} className={cn([isTopIndicator && "pt-3"])}>
              {shouldRenderIndicatorBefore && (
                <div data-sidebar-current-time-header-gap className="py-3">
                  <CurrentTimeIndicator
                    ref={setCurrentTimeIndicatorRef}
                    timezone={timezone}
                  />
                </div>
              )}
              {shouldRenderIndicatorAnchorBefore && (
                <CurrentTimeAnchor
                  registerIndicator={setCurrentTimeIndicatorRef}
                />
              )}
              <div
                data-sidebar-timeline-bucket-header
                className={cn([
                  "sticky z-20",
                  bucketHeaderTopClassName,
                  "bg-background/95 py-1 pr-1 pl-3 backdrop-blur",
                ])}
              >
                <div className="text-foreground text-base font-bold">
                  {bucket.label}
                </div>
              </div>
              {isToday ? (
                <TodayBucket
                  items={bucket.items}
                  precision={bucket.precision}
                  registerIndicator={setCurrentTimeIndicatorRef}
                  selectedSessionId={selectedSessionId}
                  selectedNodeRef={scrollSelectedSessionIntoView}
                  suppressCurrentTimeIndicator={hasActiveVisibleSession}
                  timezone={timezone}
                  selectedIds={selectedIds}
                  flatItemKeys={flatItemKeys}
                />
              ) : (
                bucket.items.map((item) => {
                  const itemKey = `${item.type}-${item.id}`;
                  const selected =
                    item.type === "session" && item.id === selectedSessionId;
                  return (
                    <TimelineItemComponent
                      key={itemKey}
                      item={item}
                      precision={bucket.precision}
                      selected={selected}
                      timezone={timezone}
                      multiSelected={selectedIds.includes(itemKey)}
                      flatItemKeys={flatItemKeys}
                      selectedNodeRef={
                        selected ? scrollSelectedSessionIntoView : undefined
                      }
                    />
                  );
                })
              )}
            </div>
          );
        })}
        {!hasToday &&
          (indicatorIndex === -1 || indicatorIndex === buckets.length) &&
          (hasActiveVisibleSession ? (
            <CurrentTimeAnchor registerIndicator={setCurrentTimeIndicatorRef} />
          ) : (
            <CurrentTimeIndicator
              ref={setCurrentTimeIndicatorRef}
              timezone={timezone}
            />
          ))}
      </div>

      {topChromeInset && (
        <div
          aria-hidden
          data-sidebar-timeline-top-fade
          className={cn([
            "pointer-events-none absolute inset-x-0 top-0 z-[15]",
            topChromeChipCount > 1
              ? "bg-background h-28"
              : topChromeChipCount === 1
                ? "bg-background h-20"
                : "from-background via-background/95 to-background/0 h-16 bg-linear-to-b from-60% via-85%",
          ])}
        />
      )}

      {(showOpenCalendarChip ||
        (topChromeInset && showCalendarSyncStatus) ||
        showTopNowChip) && (
        <div
          className={cn([
            "absolute left-1/2 z-20 flex -translate-x-1/2 transform flex-col items-center gap-2",
            topChromeInset ? "top-12" : "top-2",
          ])}
        >
          {showOpenCalendarChip && (
            <TimelineTopChip
              ariaLabel="Open calendar"
              icon={<CalendarDaysIcon size={12} />}
              onClick={handleOpenCalendar}
            >
              Open calendar
            </TimelineTopChip>
          )}
          {topChromeInset && showCalendarSyncStatus && (
            <SidebarCalendarSyncStatus status={calendarSyncStatus} />
          )}
          {showTopNowChip && (
            <TimelineNowChip direction="up" onClick={scrollToToday} />
          )}
        </div>
      )}

      {!isTodayVisible && !isScrolledPastToday && (
        <TimelineNowChip
          onClick={scrollToToday}
          direction="down"
          className={cn([
            "absolute bottom-2 left-1/2 -translate-x-1/2 transform",
            "z-20",
          ])}
        />
      )}
    </div>
  );
}

function SidebarCalendarSyncStatus({
  status,
}: {
  status: SidebarCalendarSyncStatus;
}) {
  if (status === "idle") {
    return null;
  }

  const label =
    status === "scheduled" ? "Starting calendar sync" : "Syncing calendar";

  return (
    <TimelineTopChip
      role="status"
      aria-live="polite"
      data-sidebar-calendar-sync-status
      icon={
        status === "syncing" ? (
          <Spinner size={12} />
        ) : (
          <CalendarDaysIcon size={12} />
        )
      }
    >
      {label}
    </TimelineTopChip>
  );
}

function TimelineTopChip({
  ariaLabel,
  children,
  icon,
  onClick,
  ...props
}: {
  ariaLabel?: string;
  children: ReactNode;
  icon: ReactNode;
  className?: string;
  role?: string;
  "aria-live"?: "off" | "polite" | "assertive";
  "data-sidebar-calendar-sync-status"?: true;
  onClick?: () => void;
}) {
  const className = cn([
    "border-border bg-card/95 text-muted-foreground flex h-6 items-center gap-1 rounded-full border px-2.5 text-xs font-medium shadow-xs backdrop-blur",
    onClick && "hover:bg-accent hover:text-foreground transition-colors",
    "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
    props.className,
  ]);

  if (onClick) {
    return (
      <Button
        {...props}
        aria-label={ariaLabel}
        className={className}
        onClick={onClick}
        size="sm"
        variant="outline"
      >
        <span className="flex size-3 shrink-0 items-center justify-center">
          {icon}
        </span>
        <span className="truncate">{children}</span>
      </Button>
    );
  }

  return (
    <div {...props} aria-label={ariaLabel} className={className}>
      <span className="flex size-3 shrink-0 items-center justify-center">
        {icon}
      </span>
      <span className="truncate">{children}</span>
    </div>
  );
}

function getFallbackIndicatorIndex(buckets: TimelineBucket[], nowMs: number) {
  let staleFutureBoundary: number | null = null;

  for (let index = 0; index < buckets.length; index++) {
    const bucket = buckets[index];
    const firstItem = bucket?.items[0];
    if (!bucket || !firstItem) {
      continue;
    }

    const itemDate = getItemTimestamp(firstItem);
    if (!itemDate || itemDate.getTime() >= nowMs) {
      continue;
    }

    if (isFutureBucketLabel(bucket.label)) {
      staleFutureBoundary = index + 1;
      continue;
    }

    return staleFutureBoundary ?? index;
  }

  return staleFutureBoundary ?? -1;
}

function isFutureBucketLabel(label: string) {
  return (
    label === "Tomorrow" ||
    label === "next week" ||
    label === "next month" ||
    label.startsWith("in ")
  );
}

function TimelineNowChip({
  className,
  direction,
  onClick,
}: {
  className?: string;
  direction: "up" | "down";
  onClick: () => void;
}) {
  const DirectionIcon = direction === "up" ? ArrowUpIcon : ArrowDownIcon;

  return (
    <button
      type="button"
      aria-label="Go back to now"
      className={cn([
        "border-border bg-card/95 text-foreground flex h-6 items-center gap-1 rounded-full border px-2.5 text-xs font-semibold shadow-md backdrop-blur",
        "hover:border-border hover:bg-accent hover:text-foreground transition-colors",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        className,
      ])}
      onClick={onClick}
    >
      {direction === "up" ? <DirectionIcon size={12} /> : null}
      <SunIcon size={13} className="shrink-0 text-yellow-400" />
      <span>Now</span>
      {direction === "down" ? <DirectionIcon size={12} /> : null}
    </button>
  );
}

function CurrentTimeAnchor({
  progress = 0.5,
  registerIndicator,
  variant = "seam",
}: {
  progress?: number;
  registerIndicator: (node: HTMLDivElement | null) => void;
  variant?: "seam" | "inside";
}) {
  return (
    <div
      ref={registerIndicator}
      aria-hidden
      data-sidebar-current-time-anchor
      className={cn([
        "pointer-events-none opacity-0",
        variant === "inside"
          ? "absolute inset-x-0 z-20 h-px"
          : "relative z-20 h-px",
      ])}
      style={
        variant === "inside" ? { top: `${(1 - progress) * 100}%` } : undefined
      }
    />
  );
}

function TodayBucket({
  items,
  precision,
  registerIndicator,
  selectedSessionId,
  selectedNodeRef,
  suppressCurrentTimeIndicator,
  timezone,
  selectedIds,
  flatItemKeys,
}: {
  items: TimelineItem[];
  precision: TimelinePrecision;
  registerIndicator: (node: HTMLDivElement | null) => void;
  selectedSessionId: string | undefined;
  selectedNodeRef: RefCallback<HTMLDivElement>;
  suppressCurrentTimeIndicator: boolean;
  timezone?: string;
  selectedIds: string[];
  flatItemKeys: string[];
}) {
  const currentTimeMs = useCurrentTimeMs();

  const entries = useMemo(
    () =>
      items.map((timelineItem) => ({
        item: timelineItem,
        timestamp: getItemTimestamp(timelineItem),
      })),
    [items],
  );

  const indicatorPlacement = useMemo<TimelineIndicatorPlacement>(
    // currentTimeMs in deps triggers updates as time passes,
    // but we use fresh Date() so indicator positions correctly when entries change immediately (new note).
    () => calculateTodayIndicatorPlacement(entries, new Date()),
    [entries, currentTimeMs],
  );

  const renderedEntries = useMemo(() => {
    if (entries.length === 0) {
      return (
        <>
          {suppressCurrentTimeIndicator ? (
            <CurrentTimeAnchor registerIndicator={registerIndicator} />
          ) : (
            <CurrentTimeIndicator ref={registerIndicator} timezone={timezone} />
          )}
          <div className="text-muted-foreground px-3 py-4 text-center text-sm">
            No items today
          </div>
        </>
      );
    }

    const nodes: ReactNode[] = [];

    entries.forEach((entry, index) => {
      if (
        indicatorPlacement.type === "before" &&
        index === indicatorPlacement.index
      ) {
        nodes.push(
          suppressCurrentTimeIndicator ? (
            <CurrentTimeAnchor
              key="current-time-anchor"
              registerIndicator={registerIndicator}
            />
          ) : (
            <CurrentTimeIndicator
              ref={registerIndicator}
              key="current-time-indicator"
              timezone={timezone}
            />
          ),
        );
      }

      const itemKey = `${entry.item.type}-${entry.item.id}`;
      const selected =
        entry.item.type === "session" && entry.item.id === selectedSessionId;

      const itemNode = (
        <TimelineItemComponent
          key={itemKey}
          item={entry.item}
          precision={precision}
          selected={selected}
          timezone={timezone}
          multiSelected={selectedIds.includes(itemKey)}
          flatItemKeys={flatItemKeys}
          selectedNodeRef={selected ? selectedNodeRef : undefined}
        />
      );

      if (
        indicatorPlacement.type === "inside" &&
        index === indicatorPlacement.index
      ) {
        nodes.push(
          <div key={`${itemKey}-wrapper`} className="relative">
            {suppressCurrentTimeIndicator ? (
              <CurrentTimeAnchor
                registerIndicator={registerIndicator}
                variant="inside"
                progress={indicatorPlacement.progress}
              />
            ) : (
              <CurrentTimeIndicator
                ref={registerIndicator}
                key="current-time-indicator-inside"
                timezone={timezone}
                variant="inside"
                progress={indicatorPlacement.progress}
              />
            )}
            {itemNode}
          </div>,
        );
        return;
      }

      nodes.push(itemNode);
    });

    if (indicatorPlacement.type === "after") {
      nodes.push(
        suppressCurrentTimeIndicator ? (
          <CurrentTimeAnchor
            key="current-time-anchor-end"
            registerIndicator={registerIndicator}
          />
        ) : (
          <CurrentTimeIndicator
            ref={registerIndicator}
            key="current-time-indicator-end"
            timezone={timezone}
          />
        ),
      );
    }

    return <>{nodes}</>;
  }, [
    entries,
    indicatorPlacement,
    precision,
    registerIndicator,
    selectedSessionId,
    selectedNodeRef,
    suppressCurrentTimeIndicator,
    timezone,
    selectedIds,
    flatItemKeys,
  ]);

  return renderedEntries;
}

function scrollTimelineItemIntoView(
  container: HTMLDivElement | null,
  item: HTMLDivElement,
) {
  if (!container) {
    return;
  }

  const containerRect = container.getBoundingClientRect();
  const itemRect = item.getBoundingClientRect();
  const margin = 12;
  const aboveViewport = itemRect.top < containerRect.top + margin;
  const belowViewport = itemRect.bottom > containerRect.bottom - margin;

  if (!aboveViewport && !belowViewport) {
    return;
  }

  const itemCenter =
    itemRect.top -
    containerRect.top +
    container.scrollTop +
    itemRect.height / 2;
  const targetScrollTop = Math.max(
    itemCenter - container.clientHeight * 0.45,
    0,
  );

  container.scrollTo({
    top: targetScrollTop,
    behavior: "smooth",
  });
}

function useCalendarSyncStatus(): SidebarCalendarSyncStatus {
  const manager = useManager();
  const scheduledTaskRunIds = useScheduledTaskRunIds();
  const runningTaskRunIds = useRunningTaskRunIds();
  const currentTimeMs = useCurrentTimeMs(CALENDAR_SYNC_STATUS_TICK_MS);

  return useMemo(() => {
    if (!manager) {
      return "idle";
    }

    const hasRunningSync = runningTaskRunIds?.some(
      (taskRunId) =>
        manager.getTaskRunInfo(taskRunId)?.taskId === CALENDAR_SYNC_TASK_ID,
    );
    if (hasRunningSync) {
      return "syncing";
    }

    const visibleFrom = currentTimeMs - DUE_CALENDAR_SYNC_VISIBLE_WINDOW_MS;
    const visibleUntil = currentTimeMs + DUE_CALENDAR_SYNC_VISIBLE_WINDOW_MS;
    const hasDueScheduledSync = scheduledTaskRunIds?.some((taskRunId) => {
      const info = manager.getTaskRunInfo(taskRunId);
      return (
        info?.taskId === CALENDAR_SYNC_TASK_ID &&
        info.nextTimestamp >= visibleFrom &&
        info.nextTimestamp <= visibleUntil
      );
    });

    return hasDueScheduledSync ? "scheduled" : "idle";
  }, [manager, scheduledTaskRunIds, runningTaskRunIds, currentTimeMs]);
}

function useTimelineTables(): {
  timelineEventsTable: TimelineEventsTable;
  timelineSessionsTable: TimelineSessionsTable;
} {
  const timelineEventsTable = main.UI.useResultTable(
    main.QUERIES.timelineEvents,
    main.STORE_ID,
  );
  const timelineSessionsTable = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  );

  return { timelineEventsTable, timelineSessionsTable };
}

function useTimelineData({
  isEventIgnored,
  showIgnored,
  timelineEventsTable,
  timelineSessionsTable,
  timezone,
}: {
  isEventIgnored: (
    trackingId: string | null | undefined,
    recurrenceSeriesId: string | null | undefined,
  ) => boolean;
  showIgnored: boolean;
  timelineEventsTable: TimelineEventsTable;
  timelineSessionsTable: TimelineSessionsTable;
  timezone?: string;
}): {
  buckets: TimelineBucket[];
  hasMoreFutureItems: boolean;
  hasVisibleCalendarEvents: boolean;
} {
  const windowData = useMemo(
    () =>
      deriveTimelineWindowData({
        isEventIgnored,
        showIgnored,
        timelineEventsTable,
        timelineSessionsTable,
        timezone,
      }),
    [
      isEventIgnored,
      showIgnored,
      timelineEventsTable,
      timelineSessionsTable,
      timezone,
    ],
  );
  const currentTimeMs = useSmartCurrentTime(
    windowData.timelineEventsTable,
    windowData.timelineSessionsTable,
  );

  return useMemo(() => {
    const buckets = buildTimelineBuckets({
      timelineEventsTable: windowData.timelineEventsTable,
      timelineSessionsTable: windowData.timelineSessionsTable,
      timezone,
    });

    return {
      buckets,
      hasMoreFutureItems: windowData.hasMoreFutureItems,
      hasVisibleCalendarEvents: buckets.some((bucket) =>
        bucket.items.some((item) => item.type === "event"),
      ),
    };
  }, [windowData, currentTimeMs, timezone]);
}
