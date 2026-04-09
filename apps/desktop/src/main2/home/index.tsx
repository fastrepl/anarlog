import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { format } from "@hypr/utils";

import { DateHeader } from "./date-header";
import { LazyNote } from "./lazy-note";
import { DailyNoteEditor } from "./note-editor";
import { TodayButton } from "./today-button";
import { useToday } from "./use-today";
import { WelcomeNote } from "./welcome-note";

import { useTimezone, toTz } from "~/calendar/hooks";
import { StandardTabWrapper } from "~/shared/main";
import * as main from "~/store/tinybase/store/main";

const WELCOME_DISMISSED_KEY = "daily-notes-welcome-dismissed";

export function Main2Home() {
  const today = useToday();
  const tz = useTimezone();
  const scrollRef = useRef<HTMLDivElement>(null);
  const todayRef = useRef<HTMLDivElement>(null);
  const welcomeRef = useRef<HTMLDivElement>(null);
  const [showTodayButton, setShowTodayButton] = useState(false);
  const [showWelcome, setShowWelcome] = useState(
    () => localStorage.getItem(WELCOME_DISMISSED_KEY) !== "true",
  );

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

  const handleDismissWelcome = useCallback(() => {
    localStorage.setItem(WELCOME_DISMISSED_KEY, "true");
    setShowWelcome(false);
    setTimeout(() => {
      todayRef.current?.scrollIntoView({ behavior: "smooth" });
    }, 0);
  }, []);

  const scrollToToday = useCallback(() => {
    if (showWelcome) {
      welcomeRef.current?.scrollIntoView({ behavior: "smooth" });
    } else {
      todayRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [showWelcome]);

  useEffect(() => {
    const scrollEl = scrollRef.current;
    const targetEl = showWelcome ? welcomeRef.current : todayRef.current;
    if (!scrollEl || !targetEl) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        setShowTodayButton(!entry.isIntersecting);
      },
      { root: scrollEl, threshold: 0 },
    );

    observer.observe(targetEl);
    return () => observer.disconnect();
  }, [showWelcome]);

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

            {showWelcome && (
              <div ref={welcomeRef} className="min-h-[400px]">
                <WelcomeNote onDismiss={handleDismissWelcome} />
              </div>
            )}

            {showWelcome && (
              <div className="mx-6 border-t border-neutral-200" />
            )}

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

            {import.meta.env.DEV && !showWelcome && (
              <div className="px-6 py-4">
                <DebugResetWelcome onReset={() => setShowWelcome(true)} />
              </div>
            )}
          </div>
        </div>
      </div>
    </StandardTabWrapper>
  );
}

function DebugResetWelcome({ onReset }: { onReset: () => void }) {
  const store = main.UI.useStore(main.STORE_ID);
  return (
    <button
      onClick={() => {
        localStorage.removeItem(WELCOME_DISMISSED_KEY);
        store?.delRow("daily_notes", "welcome");
        onReset();
      }}
      className="rounded bg-neutral-100 px-3 py-1.5 text-xs text-neutral-500 hover:bg-neutral-200"
    >
      [debug] Reset welcome note
    </button>
  );
}
