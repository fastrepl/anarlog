import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { format } from "@hypr/utils";

import { DailyNoteEditorContainer } from "~/home/daily-note-editor/daily-note-editor.container";
import { HomeView } from "~/home/home.view";
import { LazyNoteContainer } from "~/home/lazy-note/lazy-note.container";
import { useToday } from "~/home/use-today";
import { useTabsStore } from "~/tabs";

export function HomeContainer() {
  const today = useToday();
  const scrollRef = useRef<HTMLDivElement>(null);
  const todayRef = useRef<HTMLDivElement>(null);
  const [showTodayButton, setShowTodayButton] = useState(false);
  const openNew = useTabsStore((state) => state.openNew);

  const openDailySummary = useCallback(
    (date: string) => {
      openNew({ type: "daily-summary", date });
    },
    [openNew],
  );

  const tomorrow = useMemo(() => {
    const value = new Date();
    value.setDate(value.getDate() + 1);
    return format(value, "yyyy-MM-dd");
  }, [today]);

  const pastDates = useMemo(() => {
    return Array.from({ length: 30 }, (_, index) => {
      const value = new Date();
      value.setDate(value.getDate() - (index + 1));
      return format(value, "yyyy-MM-dd");
    });
  }, [today]);

  const scrollToToday = useCallback(() => {
    todayRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  useEffect(() => {
    const scrollElement = scrollRef.current;
    const todayElement = todayRef.current;
    if (!scrollElement || !todayElement) {
      return;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        setShowTodayButton(!(entry?.isIntersecting ?? false));
      },
      { root: scrollElement, threshold: 0 },
    );

    observer.observe(todayElement);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    window.addEventListener("scroll-to-today", scrollToToday);
    return () => window.removeEventListener("scroll-to-today", scrollToToday);
  }, [scrollToToday]);

  return (
    <HomeView
      today={today}
      tomorrow={tomorrow}
      pastDates={pastDates}
      showTodayButton={showTodayButton}
      scrollRef={scrollRef}
      todayRef={todayRef}
      onScrollToToday={scrollToToday}
      onOpenDailySummary={openDailySummary}
      todayNote={<DailyNoteEditorContainer date={today} />}
      renderPastNote={(date) => (
        <LazyNoteContainer
          date={date}
          muted
          onOpenDailySummary={openDailySummary}
        />
      )}
    />
  );
}
