import { Trans, useLingui } from "@lingui/react/macro";
import {
  CalendarOffIcon,
  CheckIcon,
  EllipsisIcon,
  Loader2Icon,
  RefreshCwIcon,
} from "lucide-react";
import { type MouseEvent, useRef, useState } from "react";

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
  onToggle: (
    calendar: CalendarItem,
    enabled: boolean,
  ) => void | Promise<unknown>;
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

  return (
    <div className={cn(["flex flex-col gap-3", className])}>
      {groups.map((group) => {
        const showHeader =
          groups.length > 1 || (group.menuItems?.length ?? 0) > 0;

        return (
          <div
            key={group.id ?? group.sourceName}
            className="flex flex-col gap-1"
          >
            {showHeader ? (
              <CalendarGroupHeader
                group={group}
                disableHoverTone={disableHoverTone}
              />
            ) : null}

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
          </div>
        );
      })}
    </div>
  );
}

function CalendarGroupHeader({
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
        "flex items-center justify-between gap-2 py-1",
        hasMenu && "group -mx-2 rounded-full px-2",
        hasMenu && !disableHoverTone && "hover:bg-accent",
      ])}
    >
      <span className="text-muted-foreground truncate text-xs font-medium">
        {group.sourceName}
      </span>
      {hasMenu ? <CalendarGroupMenuButton onClick={showContextMenu} /> : null}
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
        "text-muted-foreground shrink-0 rounded-full p-1 transition-colors",
        "pointer-events-none opacity-0 group-hover:pointer-events-auto group-hover:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100",
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
  onToggle: (enabled: boolean) => void | Promise<unknown>;
}) {
  const color = calendar.color ?? "#888";

  // Optimistic check state: the write goes through the DB queue and the
  // enabled prop only flips after the live query re-emits. The sequence
  // number keeps a stale rejection from reverting a newer toggle.
  const [pending, setPending] = useState<boolean | null>(null);
  const toggleSeqRef = useRef(0);
  if (pending !== null && pending === enabled) {
    setPending(null);
  }
  const shownEnabled = pending ?? enabled;

  return (
    <button
      type="button"
      onClick={() => {
        const next = !shownEnabled;
        const seq = ++toggleSeqRef.current;
        setPending(next);
        void Promise.resolve(onToggle(next)).catch(() => {
          if (toggleSeqRef.current === seq) {
            setPending(null);
          }
        });
      }}
      className="flex w-full items-center gap-2 py-1 pr-2 pl-0 text-left"
    >
      <div
        className={cn([
          "flex size-4 shrink-0 items-center justify-center rounded border",
          "transition-colors duration-100",
        ])}
        style={
          shownEnabled
            ? { backgroundColor: color, borderColor: color }
            : { borderColor: color }
        }
      >
        {shownEnabled && (
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
