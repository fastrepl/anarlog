import { DateHeaderView } from "~/home/date-header.view";
import { TodayButtonView } from "~/home/today-button.view";
import { StandardTabWrapperView } from "~/shared";

export function HomeView({
  today,
  tomorrow,
  pastDates,
  showTodayButton,
  scrollRef,
  todayRef,
  onScrollToToday,
  onOpenDailySummary,
  todayNote,
  renderPastNote,
}: {
  today: string;
  tomorrow: string;
  pastDates: string[];
  showTodayButton: boolean;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  todayRef: React.RefObject<HTMLDivElement | null>;
  onScrollToToday: () => void;
  onOpenDailySummary: (date: string) => void;
  todayNote: React.ReactNode;
  renderPastNote: (date: string) => React.ReactNode;
}) {
  return (
    <StandardTabWrapperView>
      <div className="relative flex-1 overflow-hidden">
        <TodayButtonView onClick={onScrollToToday} visible={showTodayButton} />
        <div ref={scrollRef} className="h-full overflow-y-auto">
          <div className="mx-auto w-full max-w-3xl">
            <div className="opacity-40">
              <DateHeaderView
                date={tomorrow}
                muted
                onOpenDailySummary={onOpenDailySummary}
              />
            </div>

            <div className="mx-6 border-t border-neutral-200" />

            <div ref={todayRef} className="flex min-h-[400px] flex-col">
              <DateHeaderView
                date={today}
                onOpenDailySummary={onOpenDailySummary}
              />
              {todayNote}
            </div>

            {pastDates.map((date) => (
              <div key={date}>
                <div className="mx-6 border-t border-neutral-200" />
                {renderPastNote(date)}
              </div>
            ))}
          </div>
        </div>
      </div>
    </StandardTabWrapperView>
  );
}
