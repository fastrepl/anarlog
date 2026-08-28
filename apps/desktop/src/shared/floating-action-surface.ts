import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";

export const floatingActionSurfaceStyles = stylex.create({
  root: {
    backgroundColor: {
      default: colors.foreground,
      ":hover": colors.foreground,
      ":is(.dark *)": "rgb(255 255 255 / 0.92)",
      ":is(.dark *):hover": "rgb(255 255 255 / 0.92)",
    },
    borderColor: {
      default: "rgb(255 255 255 / 0.2)",
      ":is(.dark *)": "rgb(0 0 0 / 0.15)",
    },
    boxShadow: {
      default:
        "inset 0 0 0 1px rgba(255, 255, 255, 0.22), 0 10px 28px rgba(0, 0, 0, 0.28)",
      ":is(.dark *)":
        "inset 0 0 0 1px rgba(0, 0, 0, 0.18), 0 16px 40px rgba(0, 0, 0, 0.52), 0 0 0 1px rgba(255, 255, 255, 0.14)",
    },
    color: {
      default: colors.background,
      ":is(.dark *)": colors.primary,
    },
  },
});
