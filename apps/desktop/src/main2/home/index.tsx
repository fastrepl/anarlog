import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { format } from "@hypr/utils";

import { DateHeader } from "./date-header";
import { LazyNote } from "./lazy-note";
import { DailyNoteEditor } from "./note-editor";
import { TodayButton } from "./today-button";
import { useToday } from "./use-today";

import { useTimezone, toTz } from "~/calendar/hooks";
import { StandardTabWrapper } from "~/shared/main";

export function Main2Home() {
  const today = useToday();
  const tz = useTimezone();
  const scrollRef = useRef<HTMLDivElement>(null);
  const todayRef = useRef<HTMLDivElement>(null);
  const [showTodayButton, setShowTodayButton] = useState(false);

  const tomorrow = useMemo(() => {
    const d = new Date();
    d.setDate(d.getDate() + 1);
    return format(toTz(d, tz), "yyyy-MM-dd");
  }, [today, tz]);

  const pastDates = useMemo(() => {
    return Array.from({ length: 30 }, (_, i) => {
      const d = new Date();
      d.setDate(d.getDate() - (i + 1));
      return format(toTz(d, tz), "yyyy-MM-dd");
    });
  }, [today, tz]);

  const scrollToToday = useCallback(() => {
    todayRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  useEffect(() => {
    const scrollEl = scrollRef.current;
    const todayEl = todayRef.current;
    if (!scrollEl || !todayEl) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        setShowTodayButton(!entry.isIntersecting);
      },
      { root: scrollEl, threshold: 0 },
    );

    observer.observe(todayEl);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    window.addEventListener("scroll-to-today", scrollToToday);
    return () => window.removeEventListener("scroll-to-today", scrollToToday);
  }, [scrollToToday]);

  return (
    <StandardTabWrapper>
      <div className="relative flex-1 overflow-hidden">
        <TodayButton onClick={scrollToToday} visible={showTodayButton} />
        <div ref={scrollRef} className="h-full overflow-y-auto">
          <div className="mx-auto w-full max-w-3xl">
            <div className="opacity-40">
              <DateHeader date={tomorrow} muted />
            </div>

            <div className="mx-6 border-t border-neutral-200" />

            <div ref={todayRef} className="min-h-[400px]">
              <DateHeader date={today} />
              <DailyNoteEditor date={today} isToday />
            </div>

            {pastDates.map((date) => (
              <div key={date}>
                <div className="mx-6 border-t border-neutral-200" />
                <LazyNote date={date} muted />
              </div>
            ))}
          </div>
        </div>
      </div>
    </StandardTabWrapper>
  );
}
