import { ArrowDown, ArrowUp, Sun } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import type { StyleXProps } from "@anlg/ui/lib/stylex";

export function UpcomingMeetingChip({
  ariaLabel,
  label,
  onClick,
}: {
  ariaLabel: string;
  label: string;
  onClick: () => void;
}) {
  return (
    <TimelineTopChip
      aria-live="polite"
      ariaLabel={ariaLabel}
      data-sidebar-upcoming-meeting-status
      sx={styles.upcomingChip}
      icon={
        <ArrowUp
          aria-hidden
          {...stylex.props(styles.directionIcon)}
          weight="bold"
        />
      }
      onClick={onClick}
    >
      {label}
    </TimelineTopChip>
  );
}

export function TimelineTopChip({
  ariaLabel,
  children,
  icon,
  onClick,
  sx,
  ...props
}: {
  ariaLabel?: string;
  children: ReactNode;
  icon: ReactNode;
  role?: string;
  sx?: StyleXProps["sx"];
  "aria-live"?: "off" | "polite" | "assertive";
  "data-sidebar-upcoming-meeting-status"?: true;
  onClick?: () => void;
}) {
  if (onClick) {
    return (
      <Button
        {...props}
        aria-label={ariaLabel}
        sx={[styles.topChip, styles.topChipInteractive, sx]}
        onClick={onClick}
        size="sm"
        variant="outline"
      >
        <span {...stylex.props(styles.iconSlot)}>{icon}</span>
        <span {...stylex.props(styles.truncate)}>{children}</span>
      </Button>
    );
  }

  return (
    <div
      {...props}
      aria-label={ariaLabel}
      {...stylex.props(styles.topChip, sx)}
    >
      <span {...stylex.props(styles.iconSlot)}>{icon}</span>
      <span {...stylex.props(styles.truncate)}>{children}</span>
    </div>
  );
}

export function TimelineNowChip({
  ariaLabel,
  children,
  direction,
  onClick,
  sx,
}: {
  ariaLabel: string;
  children: ReactNode;
  direction: "up" | "down";
  onClick: () => void;
  sx?: StyleXProps["sx"];
}) {
  const DirectionIcon = direction === "up" ? ArrowUp : ArrowDown;

  return (
    <button
      type="button"
      aria-label={ariaLabel}
      {...stylex.props(styles.nowChip, sx)}
      onClick={onClick}
    >
      {direction === "up" ? <DirectionIcon size={12} /> : null}
      <Sun size={13} {...stylex.props(styles.sunIcon)} />
      <span>{children}</span>
      {direction === "down" ? <DirectionIcon size={12} /> : null}
    </button>
  );
}

const styles = stylex.create({
  directionIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  iconSlot: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "0.75rem",
    justifyContent: "center",
    width: "0.75rem",
  },
  nowChip: {
    alignItems: "center",
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default:
        "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
      ":focus-visible": `0 0 0 2px ${colors.ring}, 0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)`,
    },
    color: colors.foreground,
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 600,
    gap: "0.25rem",
    height: "1.5rem",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    paddingInline: "0.625rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  sunIcon: {
    color: "rgb(250 204 21)",
    flexShrink: 0,
  },
  topChip: {
    alignItems: "center",
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 2px ${colors.ring}, ${shadows.sm}`,
    },
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: "0.25rem",
    height: "1.5rem",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    paddingInline: "0.625rem",
  },
  topChipInteractive: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.accent,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  truncate: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  upcomingChip: {
    backgroundColor: colors.destructive,
    borderColor: colors.destructive,
    boxShadow:
      "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    color: colors.destructiveForeground,
    justifyContent: "center",
    width: "7rem",
  },
});

export { styles as timelineChipStyles };
