import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import {
  colors,
  media,
  radii,
  shadows,
  spacing,
} from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const Textarea = React.forwardRef<
  HTMLTextAreaElement,
  React.ComponentProps<"textarea"> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  return (
    <textarea
      {...props}
      {...mergeStyleXProps([styles.root, sx], className, style)}
      ref={ref}
    />
  );
});
Textarea.displayName = "Textarea";

const styles = stylex.create({
  root: {
    "::placeholder": {
      color: colors.mutedForeground,
    },
    backgroundColor: "transparent",
    borderColor: colors.input,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 1px ${colors.ring}, ${shadows.sm}`,
    },
    cursor: {
      default: null,
      ":disabled": "not-allowed",
    },
    display: "flex",
    fontSize: {
      default: "1rem",
      [media.md]: "0.875rem",
    },
    lineHeight: {
      default: "1.5rem",
      [media.md]: "1.25rem",
    },
    minHeight: "60px",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    paddingBlock: spacing.sm,
    paddingInline: spacing.md,
    width: "100%",
  },
});

export { Textarea };
