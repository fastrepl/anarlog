import { Slot } from "@radix-ui/react-slot";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";
import { cn } from "@anlg/utils";

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>, StyleXProps {
  asChild?: boolean;
  size?: "default" | "sm" | "lg" | "icon" | null;
  variant?:
    | "default"
    | "destructive"
    | "outline"
    | "secondary"
    | "ghost"
    | "link"
    | null;
}

function buttonVariants({
  variant,
  size,
  className,
  class: classValue,
}: Pick<ButtonProps, "variant" | "size"> & {
  class?: Parameters<typeof cn>[number];
  className?: Parameters<typeof cn>[number];
} = {}) {
  return cn([
    stylex.props([
      styles.root,
      buttonVariantStyles[variant ?? "default"],
      buttonSizeStyles[size ?? "default"],
    ]).className,
    classValue,
    className,
  ]);
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, style, sx, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    const resolvedStyle = mergeStyleXProps(
      [
        styles.root,
        buttonVariantStyles[variant ?? "default"],
        buttonSizeStyles[size ?? "default"],
        sx,
      ],
      className,
      style,
    );

    return (
      <Comp
        {...props}
        ref={ref}
        className={resolvedStyle.className}
        style={resolvedStyle.style}
      />
    );
  },
);
Button.displayName = "Button";

const styles = stylex.create({
  root: {
    alignItems: "center",
    borderRadius: radii.full,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 1px ${colors.ring}`,
    },
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "inline-flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    flexShrink: {
      default: null,
      ":is(*) svg": 0,
    },
    gap: "0.5rem",
    justifyContent: "center",
    lineHeight: "1.25rem",
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
    pointerEvents: {
      default: "auto",
      ":disabled": "none",
      ":is(*) svg": "none",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    whiteSpace: "nowrap",
  },
  variantDefault: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 1px ${colors.ring}, ${shadows.sm}`,
    },
    color: colors.primaryForeground,
  },
  variantDestructive: {
    backgroundColor: {
      default: colors.destructive,
      ":hover": `color-mix(in oklab, ${colors.destructive} 90%, transparent)`,
    },
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 1px ${colors.ring}, ${shadows.sm}`,
    },
    color: colors.destructiveForeground,
  },
  variantOutline: {
    backgroundColor: {
      default: colors.background,
      ":hover": colors.accent,
    },
    borderColor: colors.input,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 1px ${colors.ring}, ${shadows.sm}`,
    },
    color: {
      default: null,
      ":hover": colors.accentForeground,
    },
  },
  variantSecondary: {
    backgroundColor: {
      default: colors.secondary,
      ":hover": `color-mix(in oklab, ${colors.secondary} 80%, transparent)`,
    },
    boxShadow: {
      default: shadows.sm,
      ":focus-visible": `0 0 0 1px ${colors.ring}, ${shadows.sm}`,
    },
    color: colors.secondaryForeground,
  },
  variantGhost: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    color: {
      default: null,
      ":hover": colors.accentForeground,
    },
  },
  variantLink: {
    color: colors.primary,
    textDecorationLine: {
      default: "none",
      ":hover": "underline",
    },
    textUnderlineOffset: "4px",
  },
  sizeDefault: {
    height: "2.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "1rem",
  },
  sizeSm: {
    fontSize: "0.75rem",
    height: "1.75rem",
    lineHeight: "1rem",
    paddingInline: "0.5rem",
  },
  sizeLg: {
    height: "2.5rem",
    paddingInline: "2rem",
  },
  sizeIcon: {
    height: "1.75rem",
    width: "1.75rem",
  },
});

const buttonVariantStyles = {
  default: styles.variantDefault,
  destructive: styles.variantDestructive,
  ghost: styles.variantGhost,
  link: styles.variantLink,
  outline: styles.variantOutline,
  secondary: styles.variantSecondary,
};

const buttonSizeStyles = {
  default: styles.sizeDefault,
  icon: styles.sizeIcon,
  lg: styles.sizeLg,
  sm: styles.sizeSm,
};

export { Button, buttonVariants };
