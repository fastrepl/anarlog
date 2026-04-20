import { useEffect, useRef, useState } from "react";

import { DailyNoteEditorContainer } from "~/home/daily-note-editor/daily-note-editor.container";
import { LazyNoteView } from "~/home/lazy-note/lazy-note.view";

export function LazyNoteContainer({
  date,
  muted,
  onOpenDailySummary,
}: {
  date: string;
  muted?: boolean;
  onOpenDailySummary?: (date: string) => void;
}) {
  const [visible, setVisible] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          setVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: "400px" },
    );

    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return (
    <LazyNoteView
      date={date}
      muted={muted}
      visible={visible}
      containerRef={ref}
      note={visible ? <DailyNoteEditorContainer date={date} /> : null}
      onOpenDailySummary={onOpenDailySummary}
    />
  );
}
