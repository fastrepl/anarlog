import { Icon } from "@iconify-icon/react";

import { cn } from "@hypr/utils";

export function DailyNoteMock() {
  return (
    <div className="border-color-brand overflow-hidden rounded-2xl border bg-stone-100 px-2 pb-2 shadow-xl">
      <div className="flex h-11 shrink-0 items-center gap-2 pl-2">
        <div className="flex gap-2">
          <div className="size-3 rounded-full bg-red-400" />
          <div className="size-3 rounded-full bg-yellow-400" />
          <div className="size-3 rounded-full bg-green-400" />
        </div>

        <div className="ml-2 flex size-7 items-center justify-center rounded-md text-neutral-700">
          <Icon icon="mdi:home-variant-outline" className="text-base" />
        </div>

        <div className="flex items-center gap-1">
          <div className="flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-neutral-500">
            <Icon
              icon="mdi:note-text-outline"
              className="shrink-0 text-xs text-neutral-500"
            />
            <span className="max-w-[120px] truncate">Team Standup</span>
          </div>
          <div className="flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-neutral-500">
            <Icon icon="mdi:note-text-outline" className="shrink-0 text-xs" />
            <span className="max-w-[120px] truncate">Design review</span>
          </div>
        </div>

        <div className="flex-1" />

        <div className="flex items-center gap-1">
          <button className="flex size-7 items-center justify-center text-neutral-500 hover:text-neutral-700">
            <Icon icon="mdi:magnify" className="text-base" />
          </button>
          <button className="flex size-7 items-center justify-center text-neutral-500 hover:text-neutral-700">
            <Icon icon="mdi:plus" className="text-base" />
          </button>
          <div className="flex size-7 items-center justify-center overflow-hidden rounded-full bg-amber-100">
            <span className="text-[10px] font-medium text-amber-800">JD</span>
          </div>
        </div>
      </div>

      <div className="scrollbar-hide surface border-color-brand h-[480px] overflow-y-hidden rounded-lg border px-8 pt-6 pb-8">
        <DayHeader label="April 17th" muted />
        <DaySeparator />

        <DayHeader label="April 16th" today />
        <div className="flex flex-col gap-1.5 pt-2 pb-6">
          <MeetingTaskRow title="Team Standup" time="9:30 AM" done />
          <MeetingTaskRow title="Design review w/ Sarah" time="11:00 AM" />
          <ActionTaskRow label="Finish mobile navigation prototype" due />
          <ActionTaskRow label="Review dashboard mockups from Victor" due />
          <MeetingTaskRow title="1:1 with Alice" time="3:00 PM" />
          <ActionTaskRow label="Respond to vendor contract from Legal" />
        </div>
        <DaySeparator />

        <DayHeader label="April 15th" muted />
        <DaySeparator />

        <DayHeader label="April 14th" muted />
      </div>
    </div>
  );
}

function DayHeader({
  label,
  today,
  muted,
}: {
  label: string;
  today?: boolean;
  muted?: boolean;
}) {
  return (
    <div className="flex items-center gap-2 py-3">
      <h4
        className={cn([
          "text-xl",
          muted
            ? "font-medium text-neutral-400"
            : "font-semibold text-neutral-900",
        ])}
      >
        {label}
      </h4>
      {today && (
        <span className="rounded-full bg-neutral-900 px-2 py-0.5 text-xs font-medium text-white">
          today
        </span>
      )}
    </div>
  );
}

function DaySeparator() {
  return <div className="my-2 border-t border-neutral-200" />;
}

function MeetingTaskRow({
  title,
  time,
  done,
}: {
  title: string;
  time: string;
  done?: boolean;
}) {
  return (
    <div
      role="button"
      className="group flex cursor-pointer items-center gap-3 rounded-md py-1.5 pr-3 hover:bg-neutral-100/70"
    >
      <TaskCheckbox done={done} />
      <Icon
        icon="mdi:calendar-blank-outline"
        className="shrink-0 text-base text-neutral-400"
      />
      <span
        className={cn([
          "flex-1 text-sm",
          done ? "text-neutral-400 line-through" : "text-neutral-800",
        ])}
      >
        {title}
      </span>
      <span className="shrink-0 font-mono text-xs text-neutral-400">
        {time}
      </span>
    </div>
  );
}

function ActionTaskRow({ label, due }: { label: string; due?: boolean }) {
  return (
    <div className="flex items-center gap-3 py-1.5 pr-3">
      <TaskCheckbox />
      <span className="flex-1 text-sm text-neutral-800">{label}</span>
      {due && (
        <span className="shrink-0 rounded-full border border-neutral-200 px-2.5 py-0.5 text-[11px] font-medium text-neutral-500">
          Due
        </span>
      )}
    </div>
  );
}

function TaskCheckbox({ done }: { done?: boolean }) {
  return (
    <span
      className={cn([
        "flex size-4 shrink-0 items-center justify-center rounded border",
        done
          ? "border-neutral-900 bg-neutral-900 text-white"
          : "border-neutral-300 bg-white",
      ])}
    >
      {done && <Icon icon="mdi:check" className="text-[10px]" />}
    </span>
  );
}
