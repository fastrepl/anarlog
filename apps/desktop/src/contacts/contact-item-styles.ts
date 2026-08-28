import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

export const contactItemStyles = stylex.create({
  active: {
    backgroundColor: colors.accent,
  },
  avatarFallback: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    height: "2rem",
    justifyContent: "center",
    width: "2rem",
  },
  avatarIcon: {
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
  body: {
    flex: "1",
    minWidth: 0,
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  inactive: {
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.accent} 50%, transparent)`,
    },
  },
  item: {
    alignItems: "center",
    borderRadius: radii.lg,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
    overflow: "hidden",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    userSelect: "none",
    width: "100%",
  },
  pin: {
    borderRadius: radii.sm,
    flexShrink: 0,
    padding: "0.25rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  pinned: {
    color: {
      default: "rgb(37 99 235)",
      ":hover": "rgb(29 78 216)",
    },
  },
  unpinned: {
    color: {
      default: `color-mix(in srgb, ${colors.mutedForeground} 70%, transparent)`,
      ":hover": colors.mutedForeground,
    },
    opacity: {
      default: 0,
      ":is([data-contact-item]:hover *)": 1,
    },
  },
});
