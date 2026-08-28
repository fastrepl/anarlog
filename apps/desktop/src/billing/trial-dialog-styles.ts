import * as stylex from "@stylexjs/stylex";

import {
  colors,
  media,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";

export const trialDialogStyles = stylex.create({
  action: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
      ":is(.dark *)": "white",
      ":is(.dark *):hover": "rgb(255 255 255 / 0.9)",
    },
    borderRadius: radii.full,
    boxShadow: shadows.sm,
    color: {
      default: colors.primaryForeground,
      ":is(.dark *)": "black",
    },
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    paddingInline: "1rem",
  },
  description: {
    color: colors.foreground,
    fontSize: "0.8125rem",
    lineHeight: 1.36,
    textAlign: "center",
    width: "100%",
  },
  footer: {
    display: "grid",
    gap: "0.5rem",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    justifyContent: {
      default: null,
      [media.sm]: "normal",
    },
  },
  footerCentered: {
    justifyContent: {
      default: null,
      [media.sm]: "center",
    },
  },
  header: {
    alignItems: "center",
    gap: "0.5rem",
    textAlign: "center",
  },
  title: {
    color: colors.foreground,
    fontSize: "0.8125rem",
    fontWeight: 600,
    letterSpacing: "normal",
    lineHeight: "1.25rem",
  },
  wideAction: {
    width: "100%",
  },
});
