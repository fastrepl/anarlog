import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  CalendarSlash,
  Check,
  CircleNotch,
  DotsThree,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { type MouseEvent, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

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
  sx?: stylex.StyleXStyles;
  isLoading?: boolean;
  disableHoverTone?: boolean;
}

export function CalendarSelection({
  groups,
  onToggle,
  onRefresh,
  sx,
  isLoading,
  disableHoverTone,
}: CalendarSelectionProps) {
  const { t } = useLingui();

  if (groups.length === 0) {
    return (
      <div {...stylex.props([styles.empty, sx])}>
        {isLoading ? (
          <>
            <CircleNotch
              {...stylex.props([styles.emptyIcon, styles.spinner])}
            />
            <p {...stylex.props(styles.emptyText)}>
              <Trans>Loading calendars...</Trans>
            </p>
          </>
        ) : (
          <>
            <CalendarSlash {...stylex.props(styles.emptyIcon)} />
            <div {...stylex.props(styles.emptyMessage)}>
              <p>
                <Trans>No calendars found</Trans>
              </p>
              {onRefresh ? (
                <button
                  type="button"
                  onClick={onRefresh}
                  {...stylex.props(styles.refreshButton)}
                  aria-label={t`Refresh calendars`}
                >
                  <ArrowsClockwise {...stylex.props(styles.smallIcon)} />
                </button>
              ) : null}
            </div>
          </>
        )}
      </div>
    );
  }

  return (
    <div {...stylex.props([styles.root, sx])}>
      {groups.map((group) => {
        const showHeader =
          groups.length > 1 || (group.menuItems?.length ?? 0) > 0;

        return (
          <div
            key={group.id ?? group.sourceName}
            {...stylex.props(styles.group)}
          >
            {showHeader ? (
              <CalendarGroupHeader
                group={group}
                disableHoverTone={disableHoverTone}
              />
            ) : null}

            <div {...stylex.props(styles.group)}>
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
      data-calendar-group
      onContextMenu={hasMenu ? showContextMenu : undefined}
      {...stylex.props([
        styles.groupHeader,
        hasMenu && styles.groupHeaderWithMenu,
        hasMenu && !disableHoverTone && styles.groupHeaderTone,
      ])}
    >
      <span {...stylex.props(styles.groupTitle)}>{group.sourceName}</span>
      {hasMenu ? <CalendarGroupMenuButton onClick={showContextMenu} /> : null}
    </div>
  );
}

function CalendarGroupMenuButton({
  onClick,
}: {
  onClick: (event: MouseEvent<HTMLButtonElement>) => void;
}) {
  const { t } = useLingui();
  return (
    <button
      type="button"
      onClick={onClick}
      {...stylex.props(styles.menuButton)}
      aria-label={t`Open calendar account actions`}
    >
      <DotsThree {...stylex.props(styles.menuIcon)} />
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
      {...stylex.props(styles.toggleRow)}
    >
      <div
        {...stylex.props([
          styles.checkbox,
          styles.checkboxColor(color, shownEnabled),
        ])}
      >
        {shownEnabled && (
          <Check {...stylex.props(styles.checkIcon)} weight="bold" />
        )}
      </div>
      <span {...stylex.props(styles.calendarTitle)}>{calendar.title}</span>
    </button>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  calendarTitle: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  checkIcon: {
    color: colors.primaryForeground,
    height: "0.75rem",
    width: "0.75rem",
  },
  checkbox: {
    alignItems: "center",
    borderRadius: "0.25rem",
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexShrink: 0,
    height: "1rem",
    justifyContent: "center",
    transitionDuration: "100ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  checkboxColor: (color: string, enabled: boolean) => ({
    backgroundColor: enabled ? color : "transparent",
    borderColor: color,
  }),
  empty: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    justifyContent: "center",
    paddingBlock: "1.5rem",
    paddingInline: "1rem",
  },
  emptyIcon: {
    color: `color-mix(in srgb, ${colors.mutedForeground} 70%, transparent)`,
    height: "1.5rem",
    marginBottom: "0.5rem",
    width: "1.5rem",
  },
  emptyMessage: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
  },
  emptyText: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  group: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
  },
  groupHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "space-between",
    paddingBlock: "0.25rem",
  },
  groupHeaderTone: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
  },
  groupHeaderWithMenu: {
    borderRadius: radii.full,
    marginInline: "-0.5rem",
    paddingInline: "0.5rem",
  },
  groupTitle: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  menuButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: colors.mutedForeground,
    flexShrink: 0,
    opacity: {
      default: 0,
      ":is([data-calendar-group]:hover *)": 1,
      ":focus-visible": 1,
    },
    padding: "0.25rem",
    pointerEvents: {
      default: "none",
      ":is([data-calendar-group]:hover *)": "auto",
      ":focus-visible": "auto",
    },
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  menuIcon: {
    height: "1rem",
    width: "1rem",
  },
  refreshButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: "0.25rem",
    color: colors.mutedForeground,
    padding: "0.25rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  smallIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  toggleRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    paddingBottom: "0.25rem",
    paddingLeft: 0,
    paddingRight: "0.5rem",
    paddingTop: "0.25rem",
    textAlign: "left",
    width: "100%",
  },
});
