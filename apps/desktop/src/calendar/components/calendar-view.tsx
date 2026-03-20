import {
  addDays,
  addMonths,
  eachDayOfInterval,
  endOfMonth,
  endOfWeek,
  format,
  isSameMonth,
  startOfMonth,
  startOfWeek,
  subMonths,
} from "date-fns";
import {
  CalendarCogIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { ButtonGroup } from "@hypr/ui/components/ui/button-group";
import { cn } from "@hypr/utils";

import { DayCell } from "./day-cell";
import { CalendarSidebarContent } from "./sidebar";

import { useCalendarData, useNow, useWeekStartsOn } from "~/calendar/hooks";

const WEEKDAY_HEADERS_SUN = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const WEEKDAY_HEADERS_MON = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

const VIEW_BREAKPOINTS = [
  { minWidth: 700, cols: 7 },
  { minWidth: 400, cols: 4 },
  { minWidth: 200, cols: 2 },
  { minWidth: 0, cols: 1 },
] as const;

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
  const now = useNow();
  const weekStartsOn = useWeekStartsOn();
  const weekOpts = useMemo(() => ({ weekStartsOn }), [weekStartsOn]);
  const [currentMonth, setCurrentMonth] = useState(now);
  const [weekStart, setWeekStart] = useState(() => startOfWeek(now, weekOpts));
  const [showSettings, setShowSettings] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const cols = useVisibleCols(containerRef);
  const calendarData = useCalendarData();

  const isMonthView = cols === 7;

  const goToPrev = useCallback(() => {
    if (isMonthView) {
      setCurrentMonth((m) => subMonths(m, 1));
    } else {
      setWeekStart((d) => addDays(d, -cols));
    }
  }, [isMonthView, cols]);

  const goToNext = useCallback(() => {
    if (isMonthView) {
      setCurrentMonth((m) => addMonths(m, 1));
    } else {
      setWeekStart((d) => addDays(d, cols));
    }
  }, [isMonthView, cols]);

  const goToToday = useCallback(() => {
    setCurrentMonth(now);
    setWeekStart(startOfWeek(now, weekOpts));
  }, [now, weekOpts]);

  const days = useMemo(() => {
    if (isMonthView) {
      const monthStart = startOfMonth(currentMonth);
      const monthEnd = endOfMonth(currentMonth);
      const calStart = startOfWeek(monthStart, weekOpts);
      const calEnd = endOfWeek(monthEnd, weekOpts);
      return eachDayOfInterval({ start: calStart, end: calEnd });
    }

    return eachDayOfInterval({
      start: weekStart,
      end: addDays(weekStart, cols - 1),
    });
  }, [currentMonth, isMonthView, cols, weekStart, weekOpts]);

  const visibleHeaders = useMemo(() => {
    if (isMonthView) {
      return weekStartsOn === 1 ? WEEKDAY_HEADERS_MON : WEEKDAY_HEADERS_SUN;
    }
    return days.slice(0, cols).map((d) => format(d, "EEE"));
  }, [isMonthView, days, cols, weekStartsOn]);

  return (
    <div className="flex h-full overflow-hidden">
      <div
        className={cn([
          "flex flex-col border-r border-neutral-200 transition-all duration-200",
          showSettings ? "w-72" : "w-0 border-r-0",
        ])}
      >
        {showSettings && (
          <>
            <div className="flex shrink-0 items-center gap-2 border-b border-neutral-200 px-2 pt-1 pb-1">
              <Button
                variant="ghost"
                size="icon"
                className="bg-neutral-200"
                onClick={() => setShowSettings(false)}
              >
                <CalendarCogIcon className="h-4 w-4" />
              </Button>
              <span className="text-sm font-semibold text-neutral-900">
                Calendars
              </span>
            </div>
            <div className="flex-1 overflow-y-auto p-3">
              <CalendarSidebarContent />
            </div>
          </>
        )}
      </div>
      <div ref={containerRef} className="flex min-w-0 flex-1 flex-col">
        <div
          className={cn([
            "flex items-center justify-between",
            "h-12 border-b border-neutral-200 py-2 pr-1 pl-3",
          ])}
        >
          <div className="flex items-center gap-2">
            {!showSettings && (
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setShowSettings(true)}
              >
                <CalendarCogIcon className="h-4 w-4" />
              </Button>
            )}
            <h2 className="text-sm font-medium text-neutral-900">
              {isMonthView
                ? format(currentMonth, "MMMM yyyy")
                : days.length > 0
                  ? format(days[0], "MMMM yyyy")
                  : ""}
            </h2>
          </div>
          <ButtonGroup>
            <Button
              variant="outline"
              size="icon"
              className="shadow-none"
              onClick={goToPrev}
            >
              <ChevronLeftIcon className="h-4 w-4" />
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="px-3 shadow-none"
              onClick={goToToday}
            >
              Today
            </Button>
            <Button
              variant="outline"
              size="icon"
              className="shadow-none"
              onClick={goToNext}
            >
              <ChevronRightIcon className="h-4 w-4" />
            </Button>
          </ButtonGroup>
        </div>

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
          className={cn([
            "grid flex-1 overflow-hidden",
            isMonthView ? "auto-rows-fr" : "grid-rows-1",
          ])}
          style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
        >
          {days.map((day) => (
            <DayCell
              key={day.toISOString()}
              day={day}
              isCurrentMonth={
                isMonthView ? isSameMonth(day, currentMonth) : true
              }
              calendarData={calendarData}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
