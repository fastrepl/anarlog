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
import { ChevronLeftIcon, ChevronRightIcon, RefreshCwIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  ButtonGroup,
  ButtonGroupSeparator,
} from "@hypr/ui/components/ui/button-group";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useSync } from "./context";
import { DayCell } from "./day-cell";

import { useCalendarData, useNow, useWeekStartsOn } from "~/calendar/hooks";
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
    <div ref={containerRef} className="flex h-full flex-col overflow-hidden">
      <div
        data-tauri-drag-region
        className={cn([
          "flex items-center justify-between",
          "h-12 border-b border-neutral-200 py-2 pr-3 pl-3 select-none",
        ])}
      >
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-semibold text-neutral-900">
            {isMonthView
              ? format(currentMonth, "MMMM yyyy")
              : format(compactVisibleStart, "MMMM yyyy")}
          </h2>
          <CalendarSyncHeaderControls />
        </div>
        <ButtonGroup
          data-tauri-drag-region="false"
          className={cn([
            "h-8 overflow-hidden rounded-full border border-neutral-200",
            "bg-white shadow-xs",
          ])}
        >
          <Button
            variant="ghost"
            size="icon"
            className="h-full w-10 rounded-none border-0 bg-transparent shadow-none hover:bg-neutral-50"
            onClick={goToPrev}
          >
            <ChevronLeftIcon className="h-4 w-4" />
          </Button>
          <ButtonGroupSeparator className="bg-neutral-200" />
          <Button
            variant="ghost"
            size="sm"
            className={cn([
              "h-full rounded-none border-0",
              "bg-transparent px-3 text-sm shadow-none hover:bg-neutral-50",
            ])}
            onClick={goToToday}
          >
            Today
          </Button>
          <ButtonGroupSeparator className="bg-neutral-200" />
          <Button
            variant="ghost"
            size="icon"
            className="h-full w-10 rounded-none border-0 bg-transparent shadow-none hover:bg-neutral-50"
            onClick={goToNext}
          >
            <ChevronRightIcon className="h-4 w-4" />
          </Button>
        </ButtonGroup>
      </div>

      {isMonthView ? (
        <>
          <div
            className="grid border-b border-neutral-200"
            style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
          >
            {visibleHeaders.map((day, i) => (
              <div
                key={`${day}-${i}`}
                className={cn([
                  "text-center text-xs font-medium",
                  "py-2",
                  i < visibleHeaders.length - 1 &&
                    "border-r border-r-neutral-200",
                  day === "Sat" || day === "Sun"
                    ? "text-neutral-400"
                    : "text-neutral-900",
                ])}
              >
                {day}
              </div>
            ))}
          </div>

          <div
            className="grid flex-1 auto-rows-fr overflow-hidden"
            style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
          >
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
          className={cn([
            "scrollbar-hide min-h-0 flex-1 overflow-x-auto overflow-y-hidden",
            "snap-x snap-mandatory overscroll-x-contain",
          ])}
          onScroll={handleCompactScroll}
        >
          <div
            className="grid h-full min-w-full grid-rows-[auto_minmax(0,1fr)]"
            style={{
              width: compactContentWidth,
              gridTemplateColumns: `repeat(${days.length}, minmax(0, 1fr))`,
            }}
          >
            {days.map((day) => {
              const label = format(day, "EEE");
              return (
                <div
                  key={`header-${day.toISOString()}`}
                  className={cn([
                    "snap-start border-r border-b border-r-neutral-200 border-b-neutral-200",
                    "py-2 text-center text-xs font-medium",
                    label === "Sat" || label === "Sun"
                      ? "text-neutral-400"
                      : "text-neutral-900",
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

function CalendarSyncHeaderControls() {
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
    cancelDebouncedSync();
    scheduleSync();
  }, [cancelDebouncedSync, scheduleSync]);

  const showSyncIndicator = showManualRefreshFeedback || status !== "idle";
  const statusText =
    status === "scheduled"
      ? "Sync scheduled"
      : showSyncIndicator
        ? "Syncing"
        : null;

  return (
    <div className="flex items-center">
      {showSyncIndicator ? (
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <span className="flex size-6 items-center justify-center text-neutral-500">
              <Spinner size={12} />
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">{statusText}</TooltipContent>
        </Tooltip>
      ) : (
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          data-tauri-drag-region="false"
          onClick={handleRefresh}
        >
          <RefreshCwIcon className="size-3.5" />
        </Button>
      )}
    </div>
  );
}
