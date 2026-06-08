import {
  ArrowDownIcon,
  ArrowUpIcon,
  CalendarDaysIcon,
  SearchIcon,
  SquarePenIcon,
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
  filterTimelineTablesUpToTomorrow,
  getItemTimestamp,
  hasTimelineItemsAfterTomorrow,
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
import { useOpenNoteDialog } from "~/shared/open-note-dialog";
import { useNewNote } from "~/shared/useNewNote";
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

const SIDEBAR_ACTIONS_REVEAL_DELAY_MS = 900;
const DUE_CALENDAR_SYNC_VISIBLE_WINDOW_MS = 1000;
const CALENDAR_SYNC_STATUS_TICK_MS = 500;
type SidebarTimelineActionId = "new-note" | "search" | "calendar";
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
  const allBuckets = useTimelineData({
    timelineEventsTable,
    timelineSessionsTable,
    timezone,
  });
  const [showIgnored, setShowIgnored] = useState(false);
  const [isScrolledToTop, setIsScrolledToTop] = useState(true);
  const [isScrolledToBottom, setIsScrolledToBottom] = useState(true);
  const [areSidebarActionsHidden, setAreSidebarActionsHidden] = useState(false);

  const { isIgnored } = useIgnoredEvents();
  const openNew = useTabs((state) => state.openNew);
  const createNewNote = useNewNote();
  const openNoteDialog = useOpenNoteDialog();

  const buckets = useMemo(() => {
    if (showIgnored) {
      return allBuckets;
    }

    return allBuckets
      .map((bucket) => ({
        ...bucket,
        items: bucket.items.filter((item) => {
          if (item.type !== "event") return true;
          return !isIgnored(
            item.data.tracking_id_event,
            item.data.recurrence_series_id,
          );
        }),
      }))
      .filter((bucket) => bucket.items.length > 0);
  }, [allBuckets, showIgnored, isIgnored, timezone]);

  const visibleTimelineEventsTable = useMemo(() => {
    if (showIgnored || !timelineEventsTable) {
      return timelineEventsTable;
    }

    return Object.fromEntries(
      Object.entries(timelineEventsTable).filter(
        ([, item]) =>
          !isIgnored(item.tracking_id_event, item.recurrence_series_id),
      ),
    );
  }, [timelineEventsTable, showIgnored, isIgnored]);

  const hasMoreFutureItems = useMemo(
    () =>
      hasTimelineItemsAfterTomorrow({
        timelineEventsTable: visibleTimelineEventsTable,
        timelineSessionsTable,
        timezone,
      }),
    [visibleTimelineEventsTable, timelineSessionsTable, timezone],
  );
  const calendarSyncStatus = useCalendarSyncStatus();
  const showCalendarSyncStatus =
    topChromeInset && calendarSyncStatus !== "idle";

  const showOpenCalendarChip =
    !topChromeInset &&
    showOpenCalendarButton &&
    isScrolledToTop &&
    hasMoreFutureItems;
  const topSpacerClassName = topChromeInset
    ? showCalendarSyncStatus
      ? "h-28"
      : hasMoreFutureItems
        ? "h-24"
        : "h-20"
    : "h-10";

  const hasToday = useMemo(
    () => buckets.some((bucket) => bucket.label === "Today"),
    [buckets],
  );
  const currentTimeMs = useCurrentTimeMs();

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
  const selectedSessionScrollFrameRef = useRef<number | null>(null);
  const previousScrollTopRef = useRef(0);
  const areSidebarActionsHiddenRef = useRef(false);
  const sidebarActionsRevealTimerRef = useRef<ReturnType<
    typeof setTimeout
  > | null>(null);
  const setSidebarActionsHidden = useCallback((hidden: boolean) => {
    areSidebarActionsHiddenRef.current = hidden;
    setAreSidebarActionsHidden(hidden);
  }, []);
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

    const clearSidebarActionsRevealTimer = () => {
      if (sidebarActionsRevealTimerRef.current !== null) {
        clearTimeout(sidebarActionsRevealTimerRef.current);
        sidebarActionsRevealTimerRef.current = null;
      }
    };

    const revealSidebarActionsSoon = () => {
      clearSidebarActionsRevealTimer();
      sidebarActionsRevealTimerRef.current = setTimeout(() => {
        setSidebarActionsHidden(false);
        sidebarActionsRevealTimerRef.current = null;
      }, SIDEBAR_ACTIONS_REVEAL_DELAY_MS);
    };

    const updateScrollPosition = () => {
      const maxScrollTop = Math.max(
        0,
        container.scrollHeight - container.clientHeight,
      );
      const nextScrollTop = container.scrollTop;
      const scrolledToTop = nextScrollTop <= 12;
      const scrollDelta = nextScrollTop - previousScrollTopRef.current;

      setIsScrolledToTop(scrolledToTop);
      setIsScrolledToBottom(maxScrollTop - nextScrollTop <= 12);

      if (topChromeInset && scrollDelta > 2 && !scrolledToTop) {
        setSidebarActionsHidden(true);
        revealSidebarActionsSoon();
      } else if (topChromeInset && (scrolledToTop || scrollDelta < -2)) {
        clearSidebarActionsRevealTimer();
        setSidebarActionsHidden(false);
      }

      previousScrollTopRef.current = nextScrollTop;

      return { scrolledToTop };
    };

    const { scrolledToTop } = updateScrollPosition();
    if (
      topChromeInset &&
      !scrolledToTop &&
      areSidebarActionsHiddenRef.current &&
      sidebarActionsRevealTimerRef.current === null
    ) {
      revealSidebarActionsSoon();
    }
    container.addEventListener("scroll", updateScrollPosition, {
      passive: true,
    });

    return () => {
      clearSidebarActionsRevealTimer();
      container.removeEventListener("scroll", updateScrollPosition);
    };
  }, [
    containerRef,
    buckets.length,
    flatItemKeys.length,
    topChromeInset,
    setSidebarActionsHidden,
  ]);

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

  const handleOpenNoteDialog = useCallback(() => {
    openNoteDialog.open();
  }, [openNoteDialog]);

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
          const shouldRenderIndicatorBefore =
            !hasToday && indicatorIndex === index;
          const isTopIndicator = shouldRenderIndicatorBefore && index === 0;

          return (
            <div key={bucket.label} className={cn([isTopIndicator && "pt-3"])}>
              {shouldRenderIndicatorBefore && (
                <CurrentTimeIndicator
                  ref={setCurrentTimeIndicatorRef}
                  timezone={timezone}
                />
              )}
              <div
                className={cn([
                  "sticky top-0 z-10",
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
          (indicatorIndex === -1 || indicatorIndex === buckets.length) && (
            <CurrentTimeIndicator
              ref={setCurrentTimeIndicatorRef}
              timezone={timezone}
            />
          )}
      </div>

      {topChromeInset && (
        <div
          aria-hidden
          data-sidebar-timeline-top-fade
          className={cn([
            "pointer-events-none absolute inset-x-0 top-0 z-[15]",
            showCalendarSyncStatus
              ? "bg-background h-28"
              : areSidebarActionsHidden
                ? "from-background via-background/95 to-background/0 h-20 bg-linear-to-b via-60%"
                : isScrolledToTop
                  ? "bg-background h-24"
                  : "from-background via-background/95 to-background/0 h-28 bg-linear-to-b via-55%",
          ])}
        />
      )}

      {topChromeInset && (
        <SidebarTimelineActions
          key={areSidebarActionsHidden ? "hidden" : "visible"}
          hidden={areSidebarActionsHidden}
          onNewNote={createNewNote}
          onOpenCalendar={handleOpenCalendar}
          onSearch={handleOpenNoteDialog}
          showCalendarAction={showOpenCalendarButton}
          calendarSyncStatus={calendarSyncStatus}
        />
      )}

      {(showOpenCalendarChip || (!isTodayVisible && isScrolledPastToday)) && (
        <div
          className={cn([
            "absolute left-1/2 z-20 flex -translate-x-1/2 transform flex-col items-center gap-2",
            topChromeInset
              ? areSidebarActionsHidden
                ? "top-10"
                : "top-24"
              : "top-2",
          ])}
        >
          {showOpenCalendarChip && (
            <Button
              onClick={handleOpenCalendar}
              size="sm"
              className={cn([
                "bg-card hover:bg-accent rounded-full",
                "border-border text-muted-foreground border",
                "flex items-center gap-1",
                "px-3",
                "shadow-xs",
              ])}
              variant="outline"
            >
              <CalendarDaysIcon size={12} />
              <span className="text-xs">Open calendar</span>
            </Button>
          )}
          {!isTodayVisible && isScrolledPastToday && (
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

function SidebarTimelineActions({
  hidden,
  onNewNote,
  onOpenCalendar,
  onSearch,
  showCalendarAction,
  calendarSyncStatus,
}: {
  hidden: boolean;
  onNewNote: () => void;
  onOpenCalendar: () => void;
  onSearch: () => void;
  showCalendarAction: boolean;
  calendarSyncStatus: SidebarCalendarSyncStatus;
}) {
  const [expandedActionId, setExpandedActionId] =
    useState<SidebarTimelineActionId | null>(null);
  const activeActionId = expandedActionId ?? "new-note";
  const showCalendarSyncStatus = calendarSyncStatus !== "idle";
  const hideActionContainer = hidden && !showCalendarSyncStatus;
  const actionItems = useMemo(
    () => [
      {
        ariaLabel: "New note",
        icon: <SquarePenIcon size={15} />,
        id: "new-note" as const,
        label: "New note",
        onClick: onNewNote,
      },
      {
        ariaLabel: "Search",
        icon: <SearchIcon size={15} />,
        id: "search" as const,
        label: "Search",
        onClick: onSearch,
      },
      ...(showCalendarAction
        ? [
            {
              ariaLabel: "Open calendar",
              icon: <CalendarDaysIcon size={15} />,
              id: "calendar" as const,
              label: "Calendar",
              onClick: onOpenCalendar,
            },
          ]
        : []),
    ],
    [onNewNote, onOpenCalendar, onSearch, showCalendarAction],
  );

  return (
    <div
      data-sidebar-timeline-actions
      className={cn([
        "absolute inset-x-0 top-10 z-30 pt-1 pb-2",
        "bg-background/95 backdrop-blur",
        "transition-[opacity,transform] duration-150 ease-out",
        hideActionContainer
          ? "pointer-events-none -translate-y-2 opacity-0"
          : "translate-y-0 opacity-100",
      ])}
    >
      <div
        data-sidebar-timeline-action-tabs
        role="toolbar"
        aria-label="Sidebar actions"
        className={cn([
          "flex w-full items-center gap-1",
          "transition-[opacity,transform] duration-150 ease-out",
          hidden
            ? "pointer-events-none -translate-y-2 opacity-0"
            : "translate-y-0 opacity-100",
        ])}
        onMouseLeave={() => setExpandedActionId(null)}
        onBlur={(event) => {
          const nextFocusedNode = event.relatedTarget as Node | null;
          if (!event.currentTarget.contains(nextFocusedNode)) {
            setExpandedActionId(null);
          }
        }}
      >
        {actionItems.map((action) => (
          <SidebarTimelineActionButton
            key={action.id}
            active={activeActionId === action.id}
            ariaLabel={action.ariaLabel}
            icon={action.icon}
            id={action.id}
            label={action.label}
            onClick={action.onClick}
            onExpand={setExpandedActionId}
          />
        ))}
      </div>
      <SidebarCalendarSyncStatus status={calendarSyncStatus} />
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
    <div
      role="status"
      aria-live="polite"
      data-sidebar-calendar-sync-status
      className={cn([
        "mt-1.5 flex h-5 items-center gap-1.5 px-3",
        "text-muted-foreground text-[11px] leading-none font-medium",
      ])}
    >
      <span className="text-muted-foreground flex size-3 shrink-0 items-center justify-center">
        {status === "syncing" ? (
          <Spinner size={12} />
        ) : (
          <CalendarDaysIcon size={12} />
        )}
      </span>
      <span className="truncate">{label}</span>
    </div>
  );
}

function SidebarTimelineActionButton({
  active,
  ariaLabel,
  icon,
  id,
  label,
  onClick,
  onExpand,
}: {
  active: boolean;
  ariaLabel: string;
  icon: ReactNode;
  id: SidebarTimelineActionId;
  label: string;
  onClick: () => void;
  onExpand: (id: SidebarTimelineActionId) => void;
}) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      data-sidebar-timeline-action={id}
      className={cn([
        "flex h-8 min-w-8 items-center overflow-hidden rounded-full",
        "border border-transparent",
        "text-muted-foreground text-sm font-medium",
        "transition-[width,flex-grow,background-color,border-color,color] duration-150 ease-out",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        active
          ? "border-border bg-card text-foreground flex-1 justify-center px-3 shadow-xs"
          : "hover:bg-card/80 hover:text-foreground w-8 flex-none justify-center px-2",
      ])}
      onClick={onClick}
      onFocus={() => onExpand(id)}
      onMouseEnter={() => onExpand(id)}
    >
      <span
        className={cn([
          "flex size-4 shrink-0 items-center justify-center",
          active ? "text-foreground" : "text-muted-foreground",
        ])}
      >
        {icon}
      </span>
      <span
        aria-hidden
        className={cn([
          "truncate whitespace-nowrap transition-[margin,max-width,opacity] duration-150 ease-out",
          active ? "ml-2 max-w-28 opacity-100" : "ml-0 max-w-0 opacity-0",
        ])}
      >
        {label}
      </span>
    </button>
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

function TodayBucket({
  items,
  precision,
  registerIndicator,
  selectedSessionId,
  selectedNodeRef,
  timezone,
  selectedIds,
  flatItemKeys,
}: {
  items: TimelineItem[];
  precision: TimelinePrecision;
  registerIndicator: (node: HTMLDivElement | null) => void;
  selectedSessionId: string | undefined;
  selectedNodeRef: RefCallback<HTMLDivElement>;
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
          <CurrentTimeIndicator ref={registerIndicator} timezone={timezone} />
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
          <CurrentTimeIndicator
            ref={registerIndicator}
            key="current-time-indicator"
            timezone={timezone}
          />,
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
            <CurrentTimeIndicator
              ref={registerIndicator}
              key="current-time-indicator-inside"
              timezone={timezone}
              variant="inside"
              progress={indicatorPlacement.progress}
            />
            {itemNode}
          </div>,
        );
        return;
      }

      nodes.push(itemNode);
    });

    if (indicatorPlacement.type === "after") {
      nodes.push(
        <CurrentTimeIndicator
          ref={registerIndicator}
          key="current-time-indicator-end"
          timezone={timezone}
        />,
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
  timelineEventsTable,
  timelineSessionsTable,
  timezone,
}: {
  timelineEventsTable: TimelineEventsTable;
  timelineSessionsTable: TimelineSessionsTable;
  timezone?: string;
}): TimelineBucket[] {
  const filteredTables = useMemo(
    () =>
      filterTimelineTablesUpToTomorrow({
        timelineEventsTable,
        timelineSessionsTable,
        timezone,
      }),
    [timelineEventsTable, timelineSessionsTable, timezone],
  );
  const currentTimeMs = useSmartCurrentTime(
    filteredTables.timelineEventsTable,
    filteredTables.timelineSessionsTable,
  );

  return useMemo(
    () =>
      buildTimelineBuckets({
        timelineEventsTable: filteredTables.timelineEventsTable,
        timelineSessionsTable: filteredTables.timelineSessionsTable,
        timezone,
      }),
    [filteredTables, currentTimeMs, timezone],
  );
}
