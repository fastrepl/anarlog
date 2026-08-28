import * as SwitchPrimitives from "@radix-ui/react-switch";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const Switch = React.forwardRef<
  React.ComponentRef<typeof SwitchPrimitives.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitives.Root> &
    StyleXProps & {
      size?: "sm" | "default" | "lg" | null;
    }
>(({ className, style, sx, size, ...props }, ref) => {
  const resolvedSize = size === undefined ? "default" : size;

  return (
    <SwitchPrimitives.Root
      {...props}
      {...mergeStyleXProps(
        [
          styles.root,
          resolvedSize === "sm" && styles.rootSm,
          resolvedSize === "default" && styles.rootDefault,
          resolvedSize === "lg" && styles.rootLg,
          sx,
        ],
        className,
        style,
      )}
      ref={ref}
    >
      <SwitchPrimitives.Thumb
        {...stylex.props([
          styles.thumb,
          resolvedSize === "sm" && styles.thumbSm,
          resolvedSize === "default" && styles.thumbDefault,
          resolvedSize === "lg" && styles.thumbLg,
        ])}
      />
    </SwitchPrimitives.Root>
  );
});
Switch.displayName = SwitchPrimitives.Root.displayName;

const styles = stylex.create({
  root: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ":is([data-state='checked'])": colors.primary,
      ":is([data-state='unchecked'])": colors.muted,
    },
    borderColor: {
      default: colors.border,
      ":is([data-state='checked'])": colors.primary,
    },
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "2px",
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.background}, 0 0 0 4px ${colors.ring}`,
    },
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "inline-flex",
    flexShrink: 0,
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
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  rootSm: {
    height: "1rem",
    width: "2rem",
  },
  rootDefault: {
    height: "1.5rem",
    width: "2.75rem",
  },
  rootLg: {
    height: "1.75rem",
    width: "3.5rem",
  },
  thumb: {
    backgroundColor: {
      default: colors.background,
      ":is([data-state='checked'])": colors.primaryForeground,
    },
    borderRadius: radii.full,
    boxShadow: shadows.lg,
    display: "block",
    pointerEvents: "none",
    transitionDuration: "150ms",
    transitionProperty: "transform, translate, scale, rotate",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  thumbSm: {
    height: "0.75rem",
    translate: {
      default: null,
      ":is([data-state='checked'])": "1rem 0",
      ":is([data-state='unchecked'])": "0 0",
    },
    width: "0.75rem",
  },
  thumbDefault: {
    height: "1.25rem",
    translate: {
      default: null,
      ":is([data-state='checked'])": "1.25rem 0",
      ":is([data-state='unchecked'])": "0 0",
    },
    width: "1.25rem",
  },
  thumbLg: {
    height: "1.5rem",
    translate: {
      default: null,
      ":is([data-state='checked'])": "1.75rem 0",
      ":is([data-state='unchecked'])": "0 0",
    },
    width: "1.5rem",
  },
});

export { Switch };
