import { useEffect, useRef, useState } from "react";

import { DateHeader } from "./date-header";
import { DailyNoteEditor } from "./note-editor";

export function LazyNote({ date }: { date: string }) {
  const [visible, setVisible] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: "400px" },
    );

    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={ref} className="min-h-[400px]">
      {visible && (
        <>
          <DateHeader date={date} isToday={false} />
          <DailyNoteEditor date={date} />
        </>
      )}
    </div>
  );
}
