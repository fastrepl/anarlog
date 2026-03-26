import {
  CalendarOffIcon,
  ChevronDown,
  CheckIcon,
  EllipsisIcon,
  Loader2Icon,
} from "lucide-react";
import { type MouseEvent, useMemo } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionHeader,
  AccordionItem,
  AccordionTriggerPrimitive,
} from "@hypr/ui/components/ui/accordion";
import { cn } from "@hypr/utils";

import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";

export interface CalendarItem {
  id: string;
  title: string;
  color: string;
  enabled: boolean;
}

export interface CalendarGroup {
  id?: string;
  sourceName: string;
  calendars: CalendarItem[];
  menuItems?: MenuItemDef[];
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
    () => groups.map((group) => group.id ?? group.sourceName),
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
        <CalendarGroupHeader group={groups[0]} />

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
        return (
          <AccordionItem
            key={group.id ?? group.sourceName}
            value={group.id ?? group.sourceName}
            className="group/group border-none px-2"
          >
            <CalendarGroupAccordionHeader
              group={group}
              disableHoverTone={disableHoverTone}
            />
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

function CalendarGroupAccordionHeader({
  group,
  disableHoverTone,
}: {
  group: CalendarGroup;
  disableHoverTone?: boolean;
}) {
  const showContextMenu = useNativeContextMenu(group.menuItems ?? []);
  const hasMenu = (group.menuItems?.length ?? 0) > 0;

  return (
    <div
      onContextMenu={hasMenu ? showContextMenu : undefined}
      className={cn([
        "group -mx-2 grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-1 rounded-md px-2",
        !disableHoverTone && "hover:bg-neutral-50",
      ])}
    >
      <AccordionHeader className="min-w-0">
        <AccordionTriggerPrimitive className="flex w-full min-w-0 cursor-pointer items-center py-2 text-left hover:no-underline">
          <span className="truncate text-xs font-medium text-neutral-600">
            {group.sourceName}
          </span>
        </AccordionTriggerPrimitive>
      </AccordionHeader>

      {hasMenu && <CalendarGroupMenuButton onClick={showContextMenu} />}

      <ChevronDown
        className={cn([
          "size-4 shrink-0 text-neutral-500 opacity-0 transition-all duration-200 group-hover:opacity-100 focus-within:opacity-100",
          "group-data-[state=open]/group:rotate-180",
        ])}
      />
    </div>
  );
}

function CalendarGroupHeader({ group }: { group: CalendarGroup }) {
  const showContextMenu = useNativeContextMenu(group.menuItems ?? []);
  const hasMenu = (group.menuItems?.length ?? 0) > 0;

  if (!hasMenu) return null;

  return (
    <div
      onContextMenu={showContextMenu}
      className="group flex items-center justify-between gap-2 py-1"
    >
      <span className="truncate text-xs font-medium text-neutral-600">
        {group.sourceName}
      </span>
      <CalendarGroupMenuButton onClick={showContextMenu} />
    </div>
  );
}

function CalendarGroupMenuButton({
  onClick,
}: {
  onClick: (event: MouseEvent<HTMLButtonElement>) => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn([
        "shrink-0 rounded p-1 text-neutral-400 transition-colors",
        "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
        "hover:bg-neutral-200 hover:text-neutral-700",
      ])}
      aria-label="Open calendar account actions"
    >
      <EllipsisIcon className="size-4" />
    </button>
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
