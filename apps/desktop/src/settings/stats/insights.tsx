import { Trans, useLingui } from "@lingui/react/macro";
import { type ComponentProps } from "react";

import { ChartLineUp } from "@anlg/ui/components/icons";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { panelSquircle } from "@anlg/ui/lib/squircle";
import { cn } from "@anlg/utils";

import type { summarizeActivity } from "./activity";

export function ConversationPatterns({
  stats,
}: {
  stats: ReturnType<typeof summarizeActivity>;
}) {
  const { t, i18n } = useLingui();
  const number = new Intl.NumberFormat(i18n.locale, {
    maximumFractionDigits: 1,
  });
  const weekdayFormat = new Intl.DateTimeFormat(i18n.locale, {
    weekday: "long",
    timeZone: "UTC",
  });
  const weekdays = stats.weekdayCounts.map((day) => ({
    ...day,
    label: weekdayFormat.format(new Date(Date.UTC(2026, 0, 4 + day.weekday))),
  }));
  const peak = Math.max(...weekdays.map((day) => day.count));
  const busiest = weekdays.filter((day) => day.count === peak);
  const hasPatterns = stats.conversations >= 5;
  const total = number.format(stats.conversations);
  const peakCount = number.format(peak);
  const busiestDay = busiest[0].label;
  const share = new Intl.NumberFormat(i18n.locale, {
    style: "percent",
    maximumFractionDigits: 0,
  }).format(stats.conversations ? peak / stats.conversations : 0);

  return (
    <section
      className="flex flex-col gap-8"
      aria-label={t`Your conversation patterns`}
    >
      <h3 className="text-sm font-medium">
        <Trans>Your conversation patterns</Trans>
      </h3>

      <InsightPanel
        className="border-border flex items-start gap-3 rounded-[20px] border p-5"
        aria-label={t`Busiest day`}
      >
        <ChartLineUp
          className="text-muted-foreground mt-0.5 size-5 shrink-0"
          aria-hidden="true"
        />
        <div className="flex flex-col gap-2">
          <h3 className="text-base font-medium">
            {!hasPatterns ? (
              <Trans>A little more history will help</Trans>
            ) : busiest.length === 1 ? (
              <Trans>Most conversations: {busiestDay}</Trans>
            ) : (
              <Trans>No single busiest day</Trans>
            )}
          </h3>
          <p className="text-muted-foreground text-sm">
            {!hasPatterns ? (
              <Trans>
                Capture at least 5 conversations in this period to see patterns.
              </Trans>
            ) : busiest.length === 1 ? (
              <Trans>
                {peakCount} of {total} conversations ({share}) started on this
                weekday in the selected period.
              </Trans>
            ) : (
              <Trans>
                Your busiest weekdays are tied at {peakCount} conversations each
                in the selected period.
              </Trans>
            )}
          </p>
        </div>
      </InsightPanel>

      {stats.conversations > 0 && (
        <>
          <section
            className="flex flex-col gap-5"
            aria-label={t`Conversations by weekday`}
          >
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <h3 className="text-sm font-medium">
                <Trans>Your week at a glance</Trans>
              </h3>
              <span className="text-muted-foreground text-xs">
                <Trans>Conversations: {total}</Trans>
              </span>
            </div>
            <ul className="flex flex-col gap-3">
              {weekdays.map((day) => (
                <li
                  key={day.weekday}
                  className="flex items-center gap-3 text-xs"
                >
                  <span className="text-muted-foreground w-24 shrink-0">
                    {day.label}
                  </span>
                  <div
                    className="bg-muted h-5 min-w-0 flex-1 overflow-hidden rounded-sm"
                    aria-hidden="true"
                  >
                    <div
                      className={cn([
                        "h-full rounded-sm",
                        hasPatterns && day.count === peak
                          ? "bg-foreground/70"
                          : "bg-foreground/20",
                      ])}
                      style={{ width: `${(day.count / peak) * 100}%` }}
                    />
                  </div>
                  <span className="w-10 shrink-0 text-end tabular-nums">
                    {number.format(day.count)}
                  </span>
                </li>
              ))}
            </ul>
          </section>

          {hasPatterns && (
            <div className="grid grid-cols-1 gap-3 min-[480px]:grid-cols-2">
              <InsightPanel className="border-border flex flex-col gap-2 rounded-[20px] border p-4">
                <h3 className="text-muted-foreground text-xs">
                  <Trans>Typical conversation length</Trans>
                </h3>
                <p className="text-2xl font-medium tabular-nums">
                  {stats.medianMinutes === null ? (
                    <Trans>Not available</Trans>
                  ) : (
                    <Trans>{number.format(stats.medianMinutes)} min</Trans>
                  )}
                </p>
                <p className="text-muted-foreground text-xs">
                  <Trans>
                    Median transcribed time. Conversations with timing:{" "}
                    {number.format(stats.timedConversations)}.
                  </Trans>
                </p>
              </InsightPanel>
              <InsightPanel className="border-border flex flex-col gap-2 rounded-[20px] border p-4">
                <h3 className="text-muted-foreground text-xs">
                  <Trans>Conversations per active day</Trans>
                </h3>
                <p className="text-2xl font-medium tabular-nums">
                  {number.format(stats.conversations / stats.conversationDays)}
                </p>
                <p className="text-muted-foreground text-xs">
                  <Trans>Average across the capture days counted above.</Trans>
                </p>
              </InsightPanel>
            </div>
          )}
        </>
      )}
      <p className="text-muted-foreground text-xs">
        <Trans>
          Based on captured conversations, including imported transcripts. Each
          conversation is counted once, on its first capture day in the selected
          period, using your calendar timezone. Deleted conversations are
          excluded.
        </Trans>
      </p>
    </section>
  );
}

function InsightPanel({ ref, ...props }: ComponentProps<"section">) {
  const squircleRef = useSquircleRef<HTMLElement>(ref, panelSquircle, {
    innerBorder: { width: 1, color: "var(--color-border)", opacity: 1 },
  });
  return <section {...props} ref={squircleRef} />;
}
