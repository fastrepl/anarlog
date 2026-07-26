import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowDownIcon,
  ArrowUpIcon,
  CalendarDaysIcon,
  SunIcon,
} from "lucide-react";
import {
  type ReactNode,
  memo,
  type RefCallback,
  type WheelEvent as ReactWheelEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useAnchor, useAutoScrollToAnchor } from "./anchor";
import { ManagedSharedSessionIdsContext, TimelineItemComponent } from "./item";
import {
  CurrentTimeIndicator,
  useCurrentTimeMs,
  useSmartCurrentTime,
} from "./realtime";
import {
  useUpcomingMeetingStatus,
  useUpcomingMeetingLabelFormatter,
} from "./upcoming-meeting";
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

import { useAuth } from "~/auth";
import { useIgnoredEvents } from "~/calendar/ignored-events";
import { useTimelineTables } from "~/calendar/queries";
import { useDeleteSession } from "~/session/hooks/useDeleteSession";
import { useDurableSharedNotes } from "~/shared-notes/cache";
import { useConfigValue } from "~/shared/config";
import { scrollElementByWheel } from "~/shared/dom/scroll-wheel";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { useTabs } from "~/store/zustand/tabs";
import { useTimelineSelection } from "~/store/zustand/timeline-selection";
import { useListener } from "~/stt/contexts";

export const TimelineView = memo(function TimelineView({
  showOpenCalendarButton = true,
  showIgnoredEvents,
  onShowIgnoredEventsChange,
  topChipsOverlapHeader = false,
  topChromeInset = false,
}: {
  showOpenCalendarButton?: boolean;
  showIgnoredEvents?: boolean;
  onShowIgnoredEventsChange?: (showIgnored: boolean) => void;
  topChipsOverlapHeader?: boolean;
  topChromeInset?: boolean;
} = {}) {
  const { t } = useLingui();
  const timezone = useConfigValue("timezone") || undefined;
  const { session } = useAuth();
  const durableSharedNotes = useDurableSharedNotes(session?.user.id);
  const managedSharedSessionIds = useMemo(
    () =>
      new Set(
        durableSharedNotes
          .filter((note) => note.manageAccess)
          .map((note) => note.sessionId),
      ),
    [durableSharedNotes],
  );
  const { timelineEventsTable, timelineSessionsTable } = useTimelineTables();
  const [uncontrolledShowIgnored, setUncontrolledShowIgnored] = useState(false);
  const showIgnored = showIgnoredEvents ?? uncontrolledShowIgnored;
  const [isScrolledToTop, setIsScrolledToTop] = useState(true);
  const [isScrolledToBottom, setIsScrolledToBottom] = useState(true);

  const { isIgnored } = useIgnoredEvents();
  const { buckets, hasMoreFutureItems } = useTimelineData({
    isEventIgnored: isIgnored,
    showIgnored,
    timelineEventsTable,
    timelineSessionsTable,
    timezone,
  });
  const openNew = useTabs((state) => state.openNew);

  const showOpenCalendarChip =
    showOpenCalendarButton && isScrolledToTop && hasMoreFutureItems;
  const reserveOpenCalendarChipSpace =
    showOpenCalendarButton && hasMoreFutureItems;

  const hasToday = useMemo(
    () => buckets.some((bucket) => bucket.label === "Today"),
    [buckets],
  );
  const indicatorTimeMs = useCurrentTimeMs();
  const formatUpcomingMeetingLabel = useUpcomingMeetingLabelFormatter();
  const upcomingMeetingStatus = useUpcomingMeetingStatus(
    buckets,
    formatUpcomingMeetingLabel,
    t`Now`,
  );
  const [isUpcomingMeetingVisible, setIsUpcomingMeetingVisible] =
    useState(false);
  const upcomingMeetingNodeRef = useRef<HTMLDivElement | null>(null);
  const setUpcomingMeetingNodeRef = useCallback<RefCallback<HTMLDivElement>>(
    (node) => {
      upcomingMeetingNodeRef.current = node;
    },
    [],
  );
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

  const selectedIds = useTimelineSelection((s) => s.selectedIds);
  const anchorId = useTimelineSelection((s) => s.anchorId);
  const selectAll = useTimelineSelection((s) => s.selectAll);
  const clearSelection = useTimelineSelection((s) => s.clear);
  const deleteSession = useDeleteSession();

  const flatItemKeys = useMemo(() => {
    const keys: string[] = [];
    for (const bucket of buckets) {
      for (const item of bucket.items) {
        keys.push(`${item.type}-${item.id}`);
      }
    }
    return keys;
  }, [buckets]);
  const flatItemKeysRef = useRef(flatItemKeys);
  flatItemKeysRef.current = flatItemKeys;
  const getFlatItemKeys = useCallback(() => flatItemKeysRef.current, []);
  const flatSessionItemKeys = useMemo(
    () => flatItemKeys.filter(isSessionItemKey),
    [flatItemKeys],
  );
  const selectAllShortcutStateRef = useRef({
    anchorId,
    flatSessionItemKeys,
    selectedIds,
    selectedSessionId,
    selectAll,
  });
  selectAllShortcutStateRef.current = {
    anchorId,
    flatSessionItemKeys,
    selectedIds,
    selectedSessionId,
    selectAll,
  };

  const {
    containerRef,
    isAnchorVisible: isTodayVisible,
    isScrolledPastAnchor: isScrolledPastToday,
    scrollToAnchor: scrollToToday,
    registerAnchor: setCurrentTimeIndicatorRef,
    anchorNode: todayAnchorNode,
  } = useAnchor();
  const showUpcomingMeetingChip =
    Boolean(upcomingMeetingStatus) && !isUpcomingMeetingVisible;
  const showTopNowChip =
    !showUpcomingMeetingChip && !isTodayVisible && isScrolledPastToday;
  const topSpacerClassName = topChromeInset
    ? reserveOpenCalendarChipSpace
      ? "h-14"
      : "h-12"
    : topChipsOverlapHeader
      ? "h-9"
      : "h-8";
  const bucketHeaderTopClassName = topChromeInset
    ? showOpenCalendarChip
      ? "top-14"
      : "top-12"
    : "top-0";
  const topChipStackTopClassName = topChromeInset
    ? "top-4"
    : topChipsOverlapHeader
      ? "top-1"
      : "top-2";
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
  const scrollToUpcomingMeeting = useCallback(() => {
    const node = upcomingMeetingNodeRef.current;
    if (!node) {
      return;
    }

    scrollTimelineItemIntoView(containerRef.current, node);
  }, [containerRef]);

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
      setIsUpcomingMeetingVisible(
        isTimelineItemVisible(container, upcomingMeetingNodeRef.current),
      );
    };

    updateScrollPosition();
    container.addEventListener("scroll", updateScrollPosition, {
      passive: true,
    });

    return () => {
      container.removeEventListener("scroll", updateScrollPosition);
    };
  }, [
    containerRef,
    buckets.length,
    flatItemKeys.length,
    upcomingMeetingStatus?.itemKey,
  ]);

  const todayBucketLength = useMemo(() => {
    const b = buckets.find((bucket) => bucket.label === "Today");
    return b?.items.length ?? 0;
  }, [buckets]);
  const autoScrollAnchorNode = hasToday ? todayAnchorNode : null;

  useAutoScrollToAnchor({
    scrollFn: scrollToToday,
    isVisible: isTodayVisible,
    anchorNode: autoScrollAnchorNode,
    deps: [todayBucketLength],
  });

  useMountEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const container = containerRef.current;

      if (
        !container ||
        container.closest("[inert], [aria-hidden='true']") ||
        event.defaultPrevented ||
        !isSelectAllShortcut(event) ||
        isTextEditingShortcutTarget(event.target) ||
        isTextEditingShortcutTarget(document.activeElement)
      ) {
        return;
      }

      const {
        anchorId,
        flatSessionItemKeys,
        selectedIds,
        selectedSessionId,
        selectAll,
      } = selectAllShortcutStateRef.current;

      if (
        !selectedSessionId ||
        flatSessionItemKeys.length === 0 ||
        !hasSidebarNoteSelectionContext({
          anchorId,
          selectedIds,
          selectedSessionId,
        })
      ) {
        return;
      }

      event.preventDefault();
      selectAll(flatSessionItemKeys);
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
    };
  });

  const indicatorIndex = useMemo(() => {
    if (hasToday) {
      return -1;
    }
    return getFallbackIndicatorIndex(buckets, Date.now());
  }, [buckets, hasToday, indicatorTimeMs]);

  const toggleShowIgnored = useCallback(() => {
    const nextShowIgnored = !showIgnored;

    if (onShowIgnoredEventsChange) {
      onShowIgnoredEventsChange(nextShowIgnored);
      return;
    }

    setUncontrolledShowIgnored(nextShowIgnored);
  }, [onShowIgnoredEventsChange, showIgnored]);

  const handleOpenCalendar = useCallback(() => {
    openNew({ type: "calendar" });
  }, [openNew]);

  const handleDeleteSelected = useCallback(() => {
    const sessionIds = selectedIds
      .filter((key) => key.startsWith("session-"))
      .map((key) => key.replace("session-", ""));

    const batchId = sessionIds.length > 1 ? crypto.randomUUID() : undefined;

    for (const sessionId of sessionIds) {
      deleteSession(sessionId, {
        batchId,
        title: timelineSessionsTable?.[sessionId]?.title ?? undefined,
      });
    }

    clearSelection();
  }, [selectedIds, deleteSession, clearSelection, timelineSessionsTable]);

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
              text: t`Delete Selected (${sessionCount})`,
              action: handleDeleteSelected,
              disabled: sessionCount === 0,
            },
          ]
        : [
            {
              id: "toggle-ignored",
              text: showIgnored
                ? t`Hide Deleted Events`
                : t`Show Deleted Events`,
              action: toggleShowIgnored,
            },
          ],
    [
      selectedIds,
      sessionCount,
      handleDeleteSelected,
      showIgnored,
      toggleShowIgnored,
      t,
    ],
  );

  const showContextMenu = useNativeContextMenu(contextMenuItems);
  const handleWheelCapture = useCallback(
    (event: ReactWheelEvent<HTMLDivElement>) => {
      const container = containerRef.current;
      const target = event.target;

      if (
        !container ||
        event.defaultPrevented ||
        (target instanceof Node && container.contains(target))
      ) {
        return;
      }

      scrollElementByWheel(container, event);
    },
    [containerRef],
  );

  return (
    <ManagedSharedSessionIdsContext.Provider value={managedSharedSessionIds}>
      <div
        data-sidebar-timeline-root
        className="relative h-full"
        onWheelCapture={handleWheelCapture}
      >
        <div
          ref={containerRef}
          data-sidebar-timeline-scroll
          onContextMenu={showContextMenu}
          className={cn([
            "scrollbar-hide flex h-full flex-col overflow-y-auto",
            "rounded-xl",
          ])}
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
              <div
                key={bucket.label}
                className={cn([isTopIndicator && "pt-3"])}
              >
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
                    "bg-background pt-0 pr-1 pb-1 pl-3",
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
                    getFlatItemKeys={getFlatItemKeys}
                    upcomingItemKey={upcomingMeetingStatus?.itemKey}
                    upcomingItemLabel={upcomingMeetingStatus?.label}
                    upcomingItemProgress={upcomingMeetingStatus?.progress}
                    upcomingItemNodeRef={setUpcomingMeetingNodeRef}
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
                        getFlatItemKeys={getFlatItemKeys}
                        selectedNodeRef={
                          selected ? scrollSelectedSessionIntoView : undefined
                        }
                        itemNodeRef={
                          itemKey === upcomingMeetingStatus?.itemKey
                            ? setUpcomingMeetingNodeRef
                            : undefined
                        }
                        isUpcoming={itemKey === upcomingMeetingStatus?.itemKey}
                        upcomingLabel={
                          itemKey === upcomingMeetingStatus?.itemKey
                            ? upcomingMeetingStatus.label
                            : undefined
                        }
                        upcomingProgress={
                          itemKey === upcomingMeetingStatus?.itemKey
                            ? upcomingMeetingStatus.progress
                            : undefined
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
              <CurrentTimeAnchor
                registerIndicator={setCurrentTimeIndicatorRef}
              />
            ) : (
              <CurrentTimeIndicator
                ref={setCurrentTimeIndicatorRef}
                timezone={timezone}
              />
            ))}
        </div>

        {!isScrolledToBottom && (
          <div
            aria-hidden
            data-sidebar-timeline-bottom-fade
            className="from-background/0 to-background pointer-events-none absolute inset-x-0 bottom-0 z-30 h-7 bg-linear-to-b"
          />
        )}

        {topChromeInset && (
          <div
            aria-hidden
            data-sidebar-timeline-top-occluder
            className="bg-background pointer-events-none absolute inset-x-0 top-0 z-10 h-12"
          />
        )}

        {(showOpenCalendarChip ||
          showUpcomingMeetingChip ||
          showTopNowChip) && (
          <div
            data-sidebar-timeline-top-chip-stack
            className={cn([
              "absolute left-1/2 z-20 flex -translate-x-1/2 transform flex-col items-center gap-2",
              topChipStackTopClassName,
            ])}
          >
            {showOpenCalendarChip && (
              <TimelineTopChip
                ariaLabel={t`Open calendar`}
                icon={<CalendarDaysIcon size={12} />}
                onClick={handleOpenCalendar}
              >
                <Trans>Open calendar</Trans>
              </TimelineTopChip>
            )}
            {upcomingMeetingStatus && showUpcomingMeetingChip && (
              <SidebarUpcomingMeetingStatus
                label={upcomingMeetingStatus.label}
                onClick={scrollToUpcomingMeeting}
                title={upcomingMeetingStatus.title}
              />
            )}
            {showTopNowChip && (
              <TimelineNowChip direction="up" onClick={scrollToToday} />
            )}
          </div>
        )}

        {!showUpcomingMeetingChip &&
          !isTodayVisible &&
          !isScrolledPastToday && (
            <TimelineNowChip
              onClick={scrollToToday}
              direction="down"
              className={cn([
                "absolute bottom-2 left-1/2 -translate-x-1/2 transform",
                "z-40",
              ])}
            />
          )}
      </div>
    </ManagedSharedSessionIdsContext.Provider>
  );
});

function SidebarUpcomingMeetingStatus({
  label,
  onClick,
  title,
}: {
  label: string;
  onClick: () => void;
  title: string;
}) {
  const { t } = useLingui();
  return (
    <TimelineTopChip
      aria-live="polite"
      ariaLabel={`${title || t`Meeting`} ${label.toLowerCase()}`}
      data-sidebar-upcoming-meeting-status
      className="border-destructive bg-destructive text-destructive-foreground w-28 justify-center shadow-md"
      icon={<ArrowUpIcon aria-hidden className="size-3" strokeWidth={2.4} />}
      onClick={onClick}
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
  "data-sidebar-upcoming-meeting-status"?: true;
  onClick?: () => void;
}) {
  const className = cn([
    "border-border bg-card text-muted-foreground flex h-6 items-center gap-1 rounded-full border px-2.5 text-xs font-medium shadow-xs",
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

function isSelectAllShortcut(event: KeyboardEvent) {
  return (
    event.key.toLowerCase() === "a" &&
    (event.metaKey || event.ctrlKey) &&
    !event.altKey &&
    !event.shiftKey
  );
}

function isSessionItemKey(key: string) {
  return key.startsWith("session-");
}

function hasSidebarNoteSelectionContext({
  anchorId,
  selectedIds,
  selectedSessionId,
}: {
  anchorId: string | null;
  selectedIds: string[];
  selectedSessionId: string;
}) {
  const currentSessionKey = `session-${selectedSessionId}`;

  return anchorId === currentSessionKey || selectedIds.some(isSessionItemKey);
}

function isTextEditingShortcutTarget(target: EventTarget | null) {
  const element =
    target instanceof Element
      ? target
      : target instanceof Node
        ? target.parentElement
        : null;

  return (
    element !== null &&
    Boolean(
      element.closest(
        [
          "input",
          "textarea",
          "select",
          "[contenteditable='true']",
          "[role='textbox']",
          ".ProseMirror",
        ].join(","),
      ),
    )
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
  const { t } = useLingui();

  return (
    <button
      type="button"
      aria-label={t`Go back to now`}
      className={cn([
        "border-border bg-card text-foreground flex h-6 items-center gap-1 rounded-full border px-2.5 text-xs font-semibold shadow-md",
        "hover:border-border hover:bg-accent hover:text-foreground transition-colors",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden",
        className,
      ])}
      onClick={onClick}
    >
      {direction === "up" ? <DirectionIcon size={12} /> : null}
      <SunIcon size={13} className="shrink-0 text-yellow-400" />
      <span>
        <Trans>Now</Trans>
      </span>
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
  getFlatItemKeys,
  upcomingItemKey,
  upcomingItemLabel,
  upcomingItemProgress,
  upcomingItemNodeRef,
}: {
  items: TimelineItem[];
  precision: TimelinePrecision;
  registerIndicator: (node: HTMLDivElement | null) => void;
  selectedSessionId: string | undefined;
  selectedNodeRef: RefCallback<HTMLDivElement>;
  suppressCurrentTimeIndicator: boolean;
  timezone?: string;
  selectedIds: string[];
  getFlatItemKeys: () => string[];
  upcomingItemKey?: string;
  upcomingItemLabel?: string;
  upcomingItemProgress?: number;
  upcomingItemNodeRef: RefCallback<HTMLDivElement>;
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
            <Trans>No items today</Trans>
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
          getFlatItemKeys={getFlatItemKeys}
          selectedNodeRef={selected ? selectedNodeRef : undefined}
          itemNodeRef={
            itemKey === upcomingItemKey ? upcomingItemNodeRef : undefined
          }
          isUpcoming={itemKey === upcomingItemKey}
          upcomingLabel={
            itemKey === upcomingItemKey ? upcomingItemLabel : undefined
          }
          upcomingProgress={
            itemKey === upcomingItemKey ? upcomingItemProgress : undefined
          }
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
    getFlatItemKeys,
    upcomingItemKey,
    upcomingItemLabel,
    upcomingItemProgress,
    upcomingItemNodeRef,
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

function isTimelineItemVisible(
  container: HTMLDivElement | null,
  item: HTMLDivElement | null,
) {
  if (!container || !item) {
    return false;
  }

  const containerRect = container.getBoundingClientRect();
  const itemRect = item.getBoundingClientRect();
  const margin = 8;

  return (
    itemRect.bottom > containerRect.top + margin &&
    itemRect.top < containerRect.bottom - margin
  );
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
