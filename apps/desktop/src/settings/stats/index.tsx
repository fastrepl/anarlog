import { Trans, useLingui } from "@lingui/react/macro";
import { type ComponentProps, useState } from "react";

import { Check, Trophy } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { chipSquircle, panelSquircle } from "@anlg/ui/lib/squircle";
import { cn } from "@anlg/utils";

import { CONVERSATION_MILESTONES, summarizeActivity } from "./activity";
import { useActivity } from "./queries";
import { ProgressBar } from "./tremor/progress-bar";
import { Tracker } from "./tremor/tracker";

import { useNow, useTimezone, useWeekStartsOn } from "~/calendar/hooks";
import { SettingsPageTitle } from "~/settings/page-title";

const ACTIVITY_COLORS = [
  "bg-muted",
  "bg-foreground/20",
  "bg-foreground/40",
  "bg-foreground/60",
  "bg-foreground/80",
];

const STATS_BORDER = {
  innerBorder: { width: 1, color: "var(--color-border)", opacity: 1 },
};

export function SettingsStats() {
  const { t, i18n } = useLingui();
  const activity = useActivity();
  const now = useNow();
  const timezone = useTimezone();
  const weekStartsOn = useWeekStartsOn();
  const [range, setRange] = useState<"all" | "30d" | "7d">("all");
  const stats = summarizeActivity(
    activity.data ?? [],
    now,
    timezone,
    weekStartsOn,
    range,
  );
  const number = new Intl.NumberFormat(i18n.locale);
  const dateFormat = new Intl.DateTimeFormat(i18n.locale, {
    dateStyle: "long",
    timeZone: timezone,
  });
  const monthFormat = new Intl.DateTimeFormat(i18n.locale, {
    month: "short",
    timeZone: timezone,
  });
  const weekdayFormat = new Intl.DateTimeFormat(i18n.locale, {
    weekday: "short",
    timeZone: timezone,
  });
  const columns = stats.days.filter((_, index) => index % 7 === 0);
  const ranges = [
    { id: "all", label: t`All time` },
    { id: "30d", label: t`30 days` },
    { id: "7d", label: t`7 days` },
  ] as const;
  const metrics = [
    { label: t`Conversations`, value: number.format(stats.conversations) },
    {
      label: t`Hours transcribed`,
      value: number.format(Math.round(stats.hours * 10) / 10),
    },
    { label: t`Active days`, value: number.format(stats.activeDays) },
  ];

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-8">
      <div className="flex flex-col gap-2">
        <SettingsPageTitle title={<Trans>Your stats</Trans>} />
        <p className="text-muted-foreground text-sm">
          <Trans>Your conversation history.</Trans>
        </p>
      </div>
      {activity.error ? (
        <p role="alert" className="text-muted-foreground text-sm">
          <Trans>
            Couldn't load your stats. Reopen this page to try again.
          </Trans>
        </p>
      ) : activity.isLoading ? (
        <p role="status" className="text-muted-foreground text-sm">
          <Trans>Loading your stats…</Trans>
        </p>
      ) : (
        <>
          <section className="flex flex-col gap-5" aria-label={t`Overview`}>
            <div className="flex items-center justify-between gap-3">
              <h3 className="text-sm font-medium">
                <Trans>Overview</Trans>
              </h3>
              <DateRangeGroup
                className="bg-muted flex gap-1 rounded-lg p-1"
                role="group"
                aria-label={t`Date range`}
              >
                {ranges.map((option) => (
                  <Button
                    key={option.id}
                    type="button"
                    variant="ghost"
                    size="sm"
                    aria-pressed={range === option.id}
                    onClick={() => setRange(option.id)}
                    className={cn([
                      "px-3 py-1.5 text-xs",
                      range === option.id
                        ? "bg-background text-foreground shadow-xs"
                        : "text-muted-foreground hover:text-foreground",
                    ])}
                  >
                    {option.label}
                  </Button>
                ))}
              </DateRangeGroup>
            </div>
            <dl className="grid grid-cols-1 gap-3 min-[480px]:grid-cols-3">
              {metrics.map((metric) => (
                <StatCard
                  key={metric.label}
                  label={metric.label}
                  value={metric.value}
                />
              ))}
            </dl>
          </section>

          <section
            className="flex flex-col gap-4"
            aria-label={t`Activity over the past year`}
          >
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <h3 className="text-sm font-medium">
                <Trans>Activity over the past year</Trans>
              </h3>
              <span className="text-muted-foreground text-xs">
                <Trans>Weekly streak: {stats.streak}</Trans>
              </span>
            </div>
            <div
              className="overflow-x-auto pb-1"
              tabIndex={0}
              role="region"
              aria-label={t`Daily conversations`}
            >
              <div className="min-w-[620px]">
                <div
                  className="text-muted-foreground mb-2 ml-10 grid auto-cols-fr grid-flow-col gap-[3px] text-[10px]"
                  aria-hidden="true"
                >
                  {columns.map((day, index) => (
                    <span
                      key={day.key}
                      className="overflow-visible whitespace-nowrap"
                    >
                      {index === 0 ||
                      day.date.getMonth() !== columns[index - 1].date.getMonth()
                        ? monthFormat.format(day.date)
                        : ""}
                    </span>
                  ))}
                </div>
                <div className="flex gap-2">
                  <div
                    className="text-muted-foreground grid w-8 shrink-0 grid-rows-7 gap-[3px] text-[9px]"
                    aria-hidden="true"
                  >
                    {stats.days.slice(0, 7).map((day, index) => (
                      <span key={day.key} className="flex items-center">
                        {index % 2 === 1 ? weekdayFormat.format(day.date) : ""}
                      </span>
                    ))}
                  </div>
                  <Tracker
                    className="flex-1"
                    aria-label={t`Daily conversations`}
                    data={stats.days.map((day) => {
                      const date = dateFormat.format(day.date);
                      const count = day.count;
                      return {
                        key: day.key,
                        color: ACTIVITY_COLORS[Math.min(count, 4)],
                        tooltip: t`${date}. Conversations: ${count}`,
                      };
                    })}
                  />
                </div>
              </div>
            </div>
            <div className="text-muted-foreground flex flex-wrap items-center justify-between gap-3 text-xs">
              <span>
                <Trans>
                  Capture a conversation each week to keep your streak going.
                </Trans>
              </span>
              <div className="flex items-center gap-1.5" aria-hidden="true">
                <span>
                  <Trans>Less</Trans>
                </span>
                {ACTIVITY_COLORS.map((color) => (
                  <span
                    key={color}
                    className={cn(["size-2.5 rounded-xs", color])}
                  />
                ))}
                <span>
                  <Trans>More</Trans>
                </span>
              </div>
            </div>
          </section>

          <MilestonePanel
            className="border-border flex flex-col gap-5 rounded-[20px] border p-5"
            aria-label={t`Milestones`}
          >
            <div className="flex items-start gap-3">
              <Trophy
                className="text-muted-foreground mt-0.5 size-5 shrink-0"
                aria-hidden="true"
              />
              <div className="flex flex-col gap-1">
                <h3 className="text-sm font-medium">
                  {stats.totalConversations === 0 ? (
                    <Trans>Capture your first conversation</Trans>
                  ) : (
                    <Trans>Next milestone</Trans>
                  )}
                </h3>
                <p className="text-muted-foreground text-xs">
                  <Trans>
                    {stats.totalConversations} of {stats.nextMilestone}{" "}
                    conversations captured
                  </Trans>
                </p>
              </div>
            </div>
            <ProgressBar
              aria-label={t`Next conversation milestone`}
              value={stats.totalConversations}
              max={stats.nextMilestone}
            />
            <div className="flex flex-wrap gap-2">
              {CONVERSATION_MILESTONES.map((target) => (
                <MilestoneBadge
                  key={target}
                  className={cn([
                    "inline-flex items-center gap-1 rounded-lg border px-2.5 py-1 text-xs tabular-nums",
                    stats.totalConversations >= target
                      ? "border-border bg-muted text-foreground"
                      : "border-border text-muted-foreground",
                  ])}
                >
                  {stats.totalConversations >= target && (
                    <Check className="size-3" aria-hidden="true" />
                  )}
                  <span className="sr-only">
                    {stats.totalConversations >= target ? (
                      <Trans>Reached:</Trans>
                    ) : (
                      <Trans>Upcoming:</Trans>
                    )}
                  </span>
                  {number.format(target)}
                </MilestoneBadge>
              ))}
            </div>
          </MilestonePanel>
          <p className="text-muted-foreground text-xs">
            <Trans>
              Includes imported transcripts. Deleted conversations are excluded.
            </Trans>
          </p>
        </>
      )}
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  const ref = useSquircleRef<HTMLDivElement>(
    undefined,
    panelSquircle,
    STATS_BORDER,
  );
  return (
    <div ref={ref} className="border-border rounded-[20px] border p-4">
      <dt className="text-muted-foreground text-xs">{label}</dt>
      <dd className="mt-2 text-3xl font-medium tracking-tight tabular-nums">
        {value}
      </dd>
    </div>
  );
}

function DateRangeGroup({ ref, ...props }: ComponentProps<"div">) {
  const squircleRef = useSquircleRef<HTMLDivElement>(ref);
  return <div {...props} ref={squircleRef} />;
}

function MilestonePanel({ ref, ...props }: ComponentProps<"section">) {
  const squircleRef = useSquircleRef<HTMLElement>(
    ref,
    panelSquircle,
    STATS_BORDER,
  );
  return <section {...props} ref={squircleRef} />;
}

function MilestoneBadge({ ref, ...props }: ComponentProps<"span">) {
  const squircleRef = useSquircleRef<HTMLSpanElement>(
    ref,
    chipSquircle,
    STATS_BORDER,
  );
  return <span {...props} ref={squircleRef} />;
}
