import { dailyNoteSectionClassName } from "~/home/constants";
import { DateHeaderView } from "~/home/date-header.view";

export function LazyNoteView({
  date,
  muted,
  visible,
  containerRef,
  note,
  onOpenDailySummary,
}: {
  date: string;
  muted?: boolean;
  visible: boolean;
  containerRef: React.RefObject<HTMLDivElement | null>;
  note?: React.ReactNode;
  onOpenDailySummary?: (date: string) => void;
}) {
  return (
    <div ref={containerRef} className={dailyNoteSectionClassName}>
      {visible ? (
        <>
          <DateHeaderView
            date={date}
            muted={muted}
            onOpenDailySummary={onOpenDailySummary}
          />
          {note}
        </>
      ) : null}
    </div>
  );
}
