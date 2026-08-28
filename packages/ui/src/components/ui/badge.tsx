import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors, radii, spacing } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";
import { cn } from "@anlg/utils";

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>, StyleXProps {
  variant?:
    | "default"
    | "secondary"
    | "destructive"
    | "outline"
    | "success"
    | null;
  size?: "default" | "sm" | "lg" | null;
  disabled?: boolean | null;
  label?: string;
}

function Badge({
  className,
  style,
  sx,
  variant,
  size,
  disabled,
  label,
  children,
  ...props
}: BadgeProps) {
  return (
    <div
      aria-label={label}
      role="status"
      {...props}
      {...mergeStyleXProps(
        [getBadgeStyles({ variant, size, disabled }), sx],
        className,
        style,
      )}
    >
      {children}
    </div>
  );
}

function getBadgeStyles({
  variant,
  size,
  disabled,
}: Pick<BadgeProps, "variant" | "size" | "disabled"> = {}) {
  const resolvedVariant = variant === undefined ? "default" : variant;
  const resolvedSize = size === undefined ? "default" : size;

  return [
    styles.root,
    resolvedVariant === "default" && styles.variantDefault,
    resolvedVariant === "secondary" && styles.variantSecondary,
    resolvedVariant === "destructive" && styles.variantDestructive,
    resolvedVariant === "outline" && styles.variantOutline,
    resolvedVariant === "success" && styles.variantSuccess,
    resolvedSize === "sm" && styles.sizeSm,
    resolvedSize === "lg" && styles.sizeLg,
    disabled === true && styles.disabled,
  ];
}

function badgeVariants({
  class: classValue,
  className,
  ...variants
}: Pick<BadgeProps, "variant" | "size" | "disabled"> & {
  class?: Parameters<typeof cn>[number];
  className?: Parameters<typeof cn>[number];
} = {}) {
  return cn([
    stylex.props(getBadgeStyles(variants)).className,
    classValue,
    className,
  ]);
}

const styles = stylex.create({
  root: {
    alignItems: "center",
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: null,
      ":focus": `0 0 0 2px white, 0 0 0 4px ${colors.ring}`,
    },
    display: "inline-flex",
    fontSize: "0.75rem",
    fontWeight: 600,
    lineHeight: "1rem",
    outline: {
      default: null,
      ":focus": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus": "2px",
    },
    paddingBlock: "0.125rem",
    paddingInline: "0.625rem",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  variantDefault: {
    backgroundColor: {
      default: colors.primary,
      ":hover": {
        default: null,
        "@media (hover: hover)": `color-mix(in oklab, ${colors.primary} 80%, transparent)`,
      },
    },
    borderColor: "transparent",
    color: colors.primaryForeground,
  },
  variantSecondary: {
    backgroundColor: {
      default: colors.secondary,
      ":hover": {
        default: null,
        "@media (hover: hover)": `color-mix(in oklab, ${colors.secondary} 80%, transparent)`,
      },
    },
    borderColor: "transparent",
    color: colors.secondaryForeground,
  },
  variantDestructive: {
    backgroundColor: {
      default: colors.destructive,
      ":hover": {
        default: null,
        "@media (hover: hover)": `color-mix(in oklab, ${colors.destructive} 80%, transparent)`,
      },
    },
    borderColor: "transparent",
    color: colors.destructiveForeground,
  },
  variantOutline: {
    color: colors.foreground,
  },
  variantSuccess: {
    backgroundColor: {
      default: "oklch(72.3% 0.219 149.579)",
      ":hover": {
        default: null,
        "@media (hover: hover)": "oklch(62.7% 0.194 149.214)",
      },
    },
    borderColor: "transparent",
    color: "white",
  },
  sizeSm: {
    fontSize: "0.75rem",
    lineHeight: "1rem",
    paddingBlock: "0.125rem",
    paddingInline: spacing.sm,
  },
  sizeLg: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: spacing.xs,
    paddingInline: spacing.md,
  },
  disabled: {
    cursor: "not-allowed",
    opacity: 0.5,
    pointerEvents: "none",
  },
});

export { Badge, badgeVariants };
