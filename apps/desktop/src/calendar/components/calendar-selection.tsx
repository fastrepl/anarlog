import { CalendarOffIcon, CheckIcon, Loader2Icon } from "lucide-react";
import { useMemo } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";
import { cn } from "@hypr/utils";

export interface CalendarItem {
  id: string;
  title: string;
  color: string;
  enabled: boolean;
}

export interface CalendarGroup {
  sourceName: string;
  calendars: CalendarItem[];
}

interface CalendarSelectionProps {
  groups: CalendarGroup[];
  onToggle: (calendar: CalendarItem, enabled: boolean) => void;
  className?: string;
  isLoading?: boolean;
  disableHoverTone?: boolean;
}

export function CalendarSelection({
  groups,
  onToggle,
  className,
  isLoading,
  disableHoverTone,
}: CalendarSelectionProps) {
  const defaultOpen = useMemo(
    () => groups.map((group) => group.sourceName),
    [groups],
  );
  const accordionKey = groups.length === 0 ? "empty" : "loaded";

  if (groups.length === 0) {
    return (
      <div
        className={cn([
          "flex flex-col items-center justify-center px-4 py-6",
          className,
        ])}
      >
        {isLoading ? (
          <>
            <Loader2Icon className="mb-2 size-6 animate-spin text-neutral-300" />
            <p className="text-xs text-neutral-500">Loading calendars…</p>
          </>
        ) : (
          <>
            <CalendarOffIcon className="mb-2 size-6 text-neutral-300" />
            <p className="text-xs text-neutral-500">No calendars found</p>
          </>
        )}
      </div>
    );
  }

  if (groups.length === 1) {
    return (
      <div className={cn(["flex flex-col gap-1 px-2", className])}>
        {groups[0].calendars.map((cal) => (
          <CalendarToggleRow
            key={cal.id}
            calendar={cal}
            enabled={cal.enabled}
            onToggle={(enabled) => onToggle(cal, enabled)}
          />
        ))}
      </div>
    );
  }

  return (
    <Accordion
      key={accordionKey}
      type="multiple"
      defaultValue={defaultOpen}
      className={cn(["divide-y", className])}
    >
      {groups.map((group) => {
        const enabledCount = group.calendars.filter((c) => c.enabled).length;

        return (
          <AccordionItem
            key={group.sourceName}
            value={group.sourceName}
            className="border-none px-2"
          >
            <AccordionTrigger
              className={cn([
                "cursor-pointer py-2 hover:no-underline",
                "[&>svg]:opacity-0 [&>svg]:transition-opacity hover:[&>svg]:opacity-100 focus-visible:[&>svg]:opacity-100",
                "-mx-2 rounded-md px-2",
                !disableHoverTone && "hover:bg-neutral-50",
              ])}
            >
              <div className="flex items-center gap-2">
                <span className="text-xs font-medium text-neutral-600">
                  {group.sourceName}
                </span>
                <span className="text-[10px] text-neutral-400 tabular-nums">
                  {enabledCount}/{group.calendars.length}
                </span>
              </div>
            </AccordionTrigger>
            <AccordionContent className="pb-2">
              <div className="flex flex-col gap-1">
                {group.calendars.map((cal) => (
                  <CalendarToggleRow
                    key={cal.id}
                    calendar={cal}
                    enabled={cal.enabled}
                    onToggle={(enabled) => onToggle(cal, enabled)}
                  />
                ))}
              </div>
            </AccordionContent>
          </AccordionItem>
        );
      })}
    </Accordion>
  );
}

function CalendarToggleRow({
  calendar,
  enabled,
  onToggle,
}: {
  calendar: CalendarItem;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
}) {
  const color = calendar.color ?? "#888";

  return (
    <button
      type="button"
      onClick={() => onToggle(!enabled)}
      className="flex w-full items-center gap-2 py-1 text-left"
    >
      <div
        className={cn([
          "flex size-4 shrink-0 items-center justify-center rounded border",
          "transition-colors duration-100",
        ])}
        style={
          enabled
            ? { backgroundColor: color, borderColor: color }
            : { borderColor: color }
        }
      >
        {enabled && <CheckIcon className="size-3 text-white" strokeWidth={3} />}
      </div>
      <span className="truncate text-sm">{calendar.title}</span>
    </button>
  );
}
