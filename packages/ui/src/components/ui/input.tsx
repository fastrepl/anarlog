import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import {
  colors,
  media,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const Input = React.forwardRef<
  HTMLInputElement,
  React.ComponentProps<"input"> & StyleXProps
>(({ className, style, sx, type, ...props }, ref) => {
  return (
    <input
      {...props}
      {...mergeStyleXProps([styles.root, sx], className, style)}
      ref={ref}
      type={type}
    />
  );
});
Input.displayName = "Input";

const styles = stylex.create({
  root: {
    "::file-selector-button": {
      backgroundColor: "transparent",
      borderWidth: 0,
      color: colors.foreground,
      fontSize: "0.875rem",
      fontWeight: 500,
    },
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
      ":focus-visible": `0 0 0 1px ${colors.ring}`,
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
    height: "2.25rem",
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
    paddingBlock: "0.25rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
});

export { Input };
