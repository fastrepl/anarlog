import { ArrowsClockwise, CaretLeft, CaretRight } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  addDays,
  addMonths,
  eachDayOfInterval,
  endOfMonth,
  endOfWeek,
  format,
  isSameMonth,
  startOfMonth,
  startOfDay,
  startOfWeek,
  subMonths,
} from "date-fns";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  ButtonGroup,
  ButtonGroupSeparator,
} from "@anlg/ui/components/ui/button-group";
import { Spinner } from "@anlg/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

import { useSync } from "./context";
import { DayCell } from "./day-cell";
import { getCalendarConnectionKey } from "./shared";

import { useBillingAccess } from "~/auth/billing-context";
import { useConnections } from "~/auth/useConnections";
import {
  useCalendarData,
  useEnabledCalendars,
  useNow,
  useSyncWhenCalendarConnectionsChange,
  useWeekStartsOn,
} from "~/calendar/hooks";
import type { CalendarSyncRange } from "~/services/calendar";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

const WEEKDAY_HEADERS_SUN = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const WEEKDAY_HEADERS_MON = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

const VIEW_BREAKPOINTS = [
  { minWidth: 700, cols: 7 },
  { minWidth: 400, cols: 4 },
  { minWidth: 200, cols: 2 },
  { minWidth: 0, cols: 1 },
] as const;

const COMPACT_SCROLL_PAST_DAYS = 42;
const COMPACT_SCROLL_FUTURE_DAYS = 42;
const VISIBLE_RANGE_SYNC_QUERY_KEY = "calendar-visible-range-sync";
const VISIBLE_RANGE_SYNC_STALE_MS = 60 * 1000;

function useVisibleCols(ref: React.RefObject<HTMLDivElement | null>) {
  const [cols, setCols] = useState(7);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new ResizeObserver((entries) => {
      const { width } = entries[0].contentRect;
      const match = VIEW_BREAKPOINTS.find((bp) => width >= bp.minWidth);
      const next = match?.cols ?? 1;
      setCols((prev) => (prev === next ? prev : next));
    });

    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);

  return cols;
}

export function CalendarView() {
  const { scheduleSync } = useSync();
  const { isPaid } = useBillingAccess();
  const { data: connections } = useConnections(isPaid);
  const connectionKey = useMemo(
    () => getCalendarConnectionKey(connections),
    [connections],
  );
  useSyncWhenCalendarConnectionsChange(connectionKey, scheduleSync);
  const now = useNow();
  const weekStartsOn = useWeekStartsOn();
  const weekOpts = useMemo(() => ({ weekStartsOn }), [weekStartsOn]);
  const [currentMonth, setCurrentMonth] = useState(now);
  const [visibleStart, setVisibleStart] = useState(() => startOfDay(now));
  const [compactVisibleStart, setCompactVisibleStart] = useState(() =>
    startOfDay(now),
  );
  const containerRef = useRef<HTMLDivElement>(null);
  const compactScrollRef = useRef<HTMLDivElement>(null);
  const compactBaseRef = useRef(startOfDay(now));
  const cols = useVisibleCols(containerRef);
  const calendarData = useCalendarData();
  const enabledCalendars = useEnabledCalendars();

  useMountEffect(() => {
    scheduleSync();
  });

  const isMonthView = cols === 7;

  const advanceCompact = useCallback(
    (direction: -1 | 1) => {
      const next = addDays(compactBaseRef.current, direction * cols);
      compactBaseRef.current = next;
      setVisibleStart(next);
    },
    [cols],
  );

  const goToPrev = useCallback(() => {
    if (isMonthView) {
      setCurrentMonth((m) => subMonths(m, 1));
    } else {
      advanceCompact(-1);
    }
  }, [isMonthView, advanceCompact]);

  const goToNext = useCallback(() => {
    if (isMonthView) {
      setCurrentMonth((m) => addMonths(m, 1));
    } else {
      advanceCompact(1);
    }
  }, [isMonthView, advanceCompact]);

  const goToToday = useCallback(() => {
    const todayStart = startOfDay(now);
    compactBaseRef.current = todayStart;
    setCurrentMonth(now);
    setVisibleStart(todayStart);
    setCompactVisibleStart(todayStart);
  }, [now]);

  const days = useMemo(() => {
    if (isMonthView) {
      const monthStart = startOfMonth(currentMonth);
      const monthEnd = endOfMonth(currentMonth);
      const calStart = startOfWeek(monthStart, weekOpts);
      const calEnd = endOfWeek(monthEnd, weekOpts);
      return eachDayOfInterval({ start: calStart, end: calEnd });
    }

    return eachDayOfInterval({
      start: addDays(visibleStart, -COMPACT_SCROLL_PAST_DAYS),
      end: addDays(visibleStart, COMPACT_SCROLL_FUTURE_DAYS - 1),
    });
  }, [currentMonth, isMonthView, visibleStart, weekOpts]);

  const visibleRange = useMemo<CalendarSyncRange | null>(() => {
    const firstDay = days[0];
    const lastDay = days[days.length - 1];
    if (!firstDay || !lastDay) return null;

    return {
      from: startOfDay(firstDay),
      to: startOfDay(addDays(lastDay, 1)),
    };
  }, [days]);

  const enabledCalendarKey = useMemo(
    () =>
      enabledCalendars
        .map((calendar) => calendar.id)
        .sort()
        .join(","),
    [enabledCalendars],
  );

  useVisibleRangeSync(visibleRange, enabledCalendarKey);

  const visibleHeaders =
    weekStartsOn === 1 ? WEEKDAY_HEADERS_MON : WEEKDAY_HEADERS_SUN;

  useEffect(() => {
    if (isMonthView) {
      return;
    }

    const el = compactScrollRef.current;
    if (el) {
      const dayWidth = el.clientWidth / cols;
      el.scrollTo({ left: COMPACT_SCROLL_PAST_DAYS * dayWidth });
    }
    compactBaseRef.current = visibleStart;
    setCompactVisibleStart(visibleStart);
  }, [isMonthView, visibleStart, cols]);

  const handleCompactScroll = useCallback(() => {
    const el = compactScrollRef.current;
    if (!el || cols <= 0) {
      return;
    }

    const dayWidth = el.clientWidth / cols;
    if (dayWidth <= 0) {
      return;
    }

    const maxStartIndex = Math.max(0, days.length - cols);
    const startIndex = Math.min(
      maxStartIndex,
      Math.max(0, Math.round(el.scrollLeft / dayWidth)),
    );
    const nextStart = startOfDay(addDays(days[0], startIndex));

    setCompactVisibleStart((prev) => {
      if (prev.getTime() === nextStart.getTime()) {
        return prev;
      }
      compactBaseRef.current = nextStart;
      return nextStart;
    });
  }, [cols, days]);

  const compactContentWidth = `${(days.length / cols) * 100}%`;

  return (
    <div ref={containerRef} {...stylex.props(styles.root)}>
      <div data-tauri-drag-region {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.headerTitle)}>
          <h2 {...stylex.props(styles.title)}>
            {isMonthView
              ? format(currentMonth, "MMMM yyyy")
              : format(compactVisibleStart, "MMMM yyyy")}
          </h2>
          <CalendarSyncHeaderControls />
        </div>
        <ButtonGroup data-tauri-drag-region="false" sx={styles.navigation}>
          <Button
            variant="ghost"
            size="icon"
            sx={styles.navigationIconButton}
            onClick={goToPrev}
          >
            <CaretLeft {...stylex.props(styles.navigationIcon)} />
          </Button>
          <ButtonGroupSeparator sx={styles.separator} />
          <Button
            variant="ghost"
            size="sm"
            sx={styles.todayButton}
            onClick={goToToday}
          >
            Today
          </Button>
          <ButtonGroupSeparator sx={styles.separator} />
          <Button
            variant="ghost"
            size="icon"
            sx={styles.navigationIconButton}
            onClick={goToNext}
          >
            <CaretRight {...stylex.props(styles.navigationIcon)} />
          </Button>
        </ButtonGroup>
      </div>

      {isMonthView ? (
        <>
          <div {...stylex.props(styles.weekdayGrid(cols))}>
            {visibleHeaders.map((day, i) => (
              <div
                key={`${day}-${i}`}
                {...stylex.props([
                  styles.weekdayHeader,
                  i < visibleHeaders.length - 1 && styles.weekdayDivider,
                  day === "Sat" || day === "Sun"
                    ? styles.weekendHeader
                    : styles.weekdayHeaderColor,
                ])}
              >
                {day}
              </div>
            ))}
          </div>

          <div {...stylex.props(styles.monthGrid(cols))}>
            {days.map((day) => (
              <DayCell
                key={day.toISOString()}
                day={day}
                isCurrentMonth={isSameMonth(day, currentMonth)}
                calendarData={calendarData}
              />
            ))}
          </div>
        </>
      ) : (
        <div
          ref={compactScrollRef}
          {...stylex.props(styles.compactScroll)}
          onScroll={handleCompactScroll}
        >
          <div
            {...stylex.props(
              styles.compactGrid(compactContentWidth, days.length),
            )}
          >
            {days.map((day) => {
              const label = format(day, "EEE");
              return (
                <div
                  key={`header-${day.toISOString()}`}
                  {...stylex.props([
                    styles.compactHeader,
                    label === "Sat" || label === "Sun"
                      ? styles.weekendHeader
                      : styles.weekdayHeaderColor,
                  ])}
                >
                  {label}
                </div>
              );
            })}
            {days.map((day) => (
              <DayCell
                key={day.toISOString()}
                day={day}
                isCurrentMonth={true}
                calendarData={calendarData}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function useVisibleRangeSync(
  range: CalendarSyncRange | null,
  enabledCalendarKey: string,
) {
  const { canSync, syncRange } = useSync();
  const from = range?.from.toISOString();
  const to = range?.to.toISOString();

  // eslint-disable-next-line @tanstack/query/exhaustive-deps -- ISO bounds and the enabled-calendar key fully identify the range; syncRange only executes it.
  useQuery({
    queryKey: [VISIBLE_RANGE_SYNC_QUERY_KEY, from, to, enabledCalendarKey],
    queryFn: async ({ signal }) => {
      if (!range) return null;
      await syncRange(range, signal);
      return null;
    },
    enabled: Boolean(range && canSync),
    staleTime: VISIBLE_RANGE_SYNC_STALE_MS,
    gcTime: 10 * VISIBLE_RANGE_SYNC_STALE_MS,
    retry: false,
  });
}

function CalendarSyncHeaderControls() {
  const queryClient = useQueryClient();
  const { status, cancelDebouncedSync, scheduleSync } = useSync();
  const refreshFeedbackTimeoutRef = useRef<ReturnType<
    typeof setTimeout
  > | null>(null);
  const [showManualRefreshFeedback, setShowManualRefreshFeedback] =
    useState(false);

  useEffect(() => {
    return () => {
      if (refreshFeedbackTimeoutRef.current) {
        clearTimeout(refreshFeedbackTimeoutRef.current);
      }
    };
  }, []);

  const handleRefresh = useCallback(() => {
    if (refreshFeedbackTimeoutRef.current) {
      clearTimeout(refreshFeedbackTimeoutRef.current);
    }
    setShowManualRefreshFeedback(true);
    refreshFeedbackTimeoutRef.current = setTimeout(() => {
      refreshFeedbackTimeoutRef.current = null;
      setShowManualRefreshFeedback(false);
    }, 1500);
    void queryClient.invalidateQueries({
      queryKey: [VISIBLE_RANGE_SYNC_QUERY_KEY],
    });
    cancelDebouncedSync();
    scheduleSync();
  }, [cancelDebouncedSync, queryClient, scheduleSync]);

  const showSyncIndicator = showManualRefreshFeedback || status !== "idle";
  const statusText =
    status === "scheduled"
      ? "Sync scheduled"
      : showSyncIndicator
        ? "Syncing"
        : null;

  return (
    <div {...stylex.props(styles.syncControls)}>
      {showSyncIndicator ? (
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span {...stylex.props(styles.syncIndicator)}>
              <Spinner size={12} />
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">{statusText}</TooltipContent>
        </Tooltip>
      ) : (
        <Button
          variant="ghost"
          size="icon"
          sx={styles.syncButton}
          data-tauri-drag-region="false"
          onClick={handleRefresh}
        >
          <ArrowsClockwise {...stylex.props(styles.navigationIcon)} />
        </Button>
      )}
    </div>
  );
}

const styles = stylex.create({
  compactGrid: (width: string, columns: number) => ({
    display: "grid",
    gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
    gridTemplateRows: "auto minmax(0, 1fr)",
    height: "100%",
    minWidth: "100%",
    width,
  }),
  compactHeader: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderRightColor: colors.border,
    borderRightStyle: "solid",
    borderRightWidth: "1px",
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
    paddingBlock: "0.5rem",
    scrollSnapAlign: "start",
    textAlign: "center",
  },
  compactScroll: {
    display: {
      default: null,
      "::-webkit-scrollbar": "none",
    },
    flex: "1",
    minHeight: 0,
    overflowX: "auto",
    overflowY: "hidden",
    overscrollBehaviorX: "contain",
    scrollbarWidth: "none",
    scrollSnapType: "x mandatory",
  },
  header: {
    alignItems: "center",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    height: "3rem",
    justifyContent: "space-between",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    userSelect: "none",
  },
  headerTitle: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  monthGrid: (columns: number) => ({
    display: "grid",
    flex: "1",
    gridAutoRows: "1fr",
    gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
    overflow: "hidden",
  }),
  navigation: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    height: "1.75rem",
    overflow: "hidden",
  },
  navigationIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  navigationIconButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: 0,
    borderWidth: 0,
    boxShadow: "none",
    height: "100%",
    width: "1.75rem",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
  },
  separator: {
    backgroundColor: colors.accent,
  },
  syncButton: {
    height: "1.5rem",
    width: "1.5rem",
  },
  syncControls: {
    alignItems: "center",
    display: "flex",
  },
  syncIndicator: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    height: "1.5rem",
    justifyContent: "center",
    width: "1.5rem",
  },
  title: {
    color: colors.foreground,
    fontSize: "0.875rem",
    fontWeight: 600,
    lineHeight: "1.25rem",
  },
  todayButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: 0,
    borderWidth: 0,
    boxShadow: "none",
    fontSize: "0.75rem",
    height: "100%",
    paddingInline: "0.5rem",
  },
  weekdayDivider: {
    borderRightColor: colors.border,
    borderRightStyle: "solid",
    borderRightWidth: "1px",
  },
  weekdayGrid: (columns: number) => ({
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "grid",
    gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
  }),
  weekdayHeader: {
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
    paddingBlock: "0.5rem",
    textAlign: "center",
  },
  weekdayHeaderColor: {
    color: colors.foreground,
  },
  weekendHeader: {
    color: colors.mutedForeground,
  },
});
