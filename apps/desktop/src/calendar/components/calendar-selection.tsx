import { Trans, useLingui } from "@lingui/react/macro";
import {
  CalendarOffIcon,
  ChevronDown,
  CheckIcon,
  EllipsisIcon,
  Loader2Icon,
  RefreshCwIcon,
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
  onRefresh?: () => void;
  className?: string;
  isLoading?: boolean;
  disableHoverTone?: boolean;
}

export function CalendarSelection({
  groups,
  onToggle,
  onRefresh,
  className,
  isLoading,
  disableHoverTone,
}: CalendarSelectionProps) {
  const { t } = useLingui();
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
            <Loader2Icon className="text-muted-foreground/70 mb-2 size-6 animate-spin" />
            <p className="text-muted-foreground text-xs">
              <Trans>Loading calendars...</Trans>
            </p>
          </>
        ) : (
          <>
            <CalendarOffIcon className="text-muted-foreground/70 mb-2 size-6" />
            <div className="text-muted-foreground flex items-center gap-1 text-xs">
              <p>
                <Trans>No calendars found</Trans>
              </p>
              {onRefresh ? (
                <button
                  type="button"
                  onClick={onRefresh}
                  className="text-muted-foreground hover:bg-accent hover:text-muted-foreground rounded p-1 transition-colors"
                  aria-label={t`Refresh calendars`}
                >
                  <RefreshCwIcon className="size-3" />
                </button>
              ) : null}
            </div>
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
  const hoverPadding = hasMenu
    ? "group-hover/calendar-group-row:pr-12 group-focus-within/calendar-group-row:pr-12"
    : "group-hover/calendar-group-row:pr-6 group-focus-within/calendar-group-row:pr-6";

  return (
    <div
      onContextMenu={hasMenu ? showContextMenu : undefined}
      className={cn([
        "group/calendar-group-row relative -mx-2 flex items-center rounded-md px-2",
        !disableHoverTone && "hover:bg-accent",
      ])}
    >
      <AccordionHeader className="min-w-0 flex-1">
        <AccordionTriggerPrimitive
          className={cn([
            "flex w-full min-w-0 cursor-pointer items-center py-2 pr-0 text-left transition-[padding] duration-150 hover:no-underline",
            hoverPadding,
          ])}
        >
          <span className="text-muted-foreground truncate text-xs font-medium">
            {group.sourceName}
          </span>
        </AccordionTriggerPrimitive>
      </AccordionHeader>

      <div
        className={cn([
          "pointer-events-none absolute top-1/2 right-1 flex -translate-y-1/2 items-center gap-0.5 opacity-0 transition-opacity duration-150",
          "group-focus-within/calendar-group-row:opacity-100 group-hover/calendar-group-row:opacity-100",
        ])}
      >
        {hasMenu && (
          <CalendarGroupMenuButton
            onClick={showContextMenu}
            className="pointer-events-none opacity-100 group-focus-within/calendar-group-row:pointer-events-auto group-hover/calendar-group-row:pointer-events-auto"
          />
        )}

        <ChevronDown
          className={cn([
            "text-muted-foreground pointer-events-none size-4 shrink-0 transition-transform duration-200",
            "group-data-[state=open]/group:rotate-180",
          ])}
        />
      </div>
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
      <span className="text-muted-foreground truncate text-xs font-medium">
        {group.sourceName}
      </span>
      <CalendarGroupMenuButton onClick={showContextMenu} />
    </div>
  );
}

function CalendarGroupMenuButton({
  onClick,
  className,
}: {
  onClick: (event: MouseEvent<HTMLButtonElement>) => void;
  className?: string;
}) {
  const { t } = useLingui();
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn([
        "text-muted-foreground shrink-0 rounded p-1 transition-colors",
        "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
        "hover:bg-accent hover:text-muted-foreground",
        className,
      ])}
      aria-label={t`Open calendar account actions`}
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
        {enabled && (
          <CheckIcon
            className="text-primary-foreground size-3"
            strokeWidth={3}
          />
        )}
      </div>
      <span className="truncate text-sm">{calendar.title}</span>
    </button>
  );
}
