import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

export const accountStyles = stylex.create({
  card: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "24px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 18px 50px rgb(24 22 19 / 0.08)",
    overflow: "hidden",
  },
  pill: {
    alignItems: "center",
    borderRadius: radii.full,
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "flex",
    fontSize: ".875rem",
    fontWeight: 500,
    height: "2.25rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    paddingInline: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color, border-color, color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  pillPrimary: {
    backgroundColor: {
      default: colors.foreground,
      ":hover": colors.mutedForeground,
    },
    color: colors.primaryForeground,
  },
  pillSecondary: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.muted,
    },
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.foreground,
  },
  pillDanger: {
    backgroundColor: colors.card,
    borderColor: {
      default: colors.alertBorder,
      ":hover": colors.destructive,
    },
    borderStyle: "solid",
    borderWidth: "1px",
    color: {
      default: colors.alertForeground,
      ":hover": colors.destructive,
    },
  },
  menuTrigger: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.muted,
      ":is([data-state='open'])": colors.muted,
    },
    borderRadius: radii.full,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "flex",
    flexShrink: 0,
    height: "2.25rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    outline: {
      default: null,
      ":focus-visible": `2px solid ${colors.mutedForeground}`,
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    transitionDuration: "150ms",
    transitionProperty: "background-color, color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "2.25rem",
  },
});
