import { Check } from "@phosphor-icons/react";
import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const Checkbox = React.forwardRef<
  React.ComponentRef<typeof CheckboxPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root> & StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <CheckboxPrimitive.Root
    {...props}
    {...mergeStyleXProps([styles.root, sx], className, style)}
    ref={ref}
  >
    <CheckboxPrimitive.Indicator {...stylex.props(styles.indicator)}>
      <Check {...stylex.props(styles.icon)} />
    </CheckboxPrimitive.Indicator>
  </CheckboxPrimitive.Root>
));
Checkbox.displayName = CheckboxPrimitive.Root.displayName;

const styles = stylex.create({
  root: {
    backgroundColor: {
      default: null,
      ":is([data-state='checked'])": colors.primary,
    },
    borderColor: colors.primary,
    borderRadius: "0.125rem",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 1px ${colors.ring}, ${shadows.sm}`,
    },
    color: {
      default: null,
      ":is([data-state='checked'])": colors.primaryForeground,
    },
    cursor: {
      default: null,
      ":disabled": "not-allowed",
    },
    flexShrink: 0,
    height: "1rem",
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
    width: "1rem",
  },
  indicator: {
    alignItems: "center",
    color: "currentColor",
    display: "flex",
    justifyContent: "center",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
});

export { Checkbox };
