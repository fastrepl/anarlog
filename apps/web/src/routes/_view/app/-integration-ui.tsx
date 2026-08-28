import * as stylex from "@stylexjs/stylex";
import type { ButtonHTMLAttributes, ReactNode } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const styles = stylex.create({
  style1: {
    display: "flex",
    minHeight: "100vh",
    alignItems: "center",
    justifyContent: "center",
    padding: "1.5rem",
  },
  style2: {
    display: "flex",
    width: "100%",
    maxWidth: "28rem",
    flexDirection: "column",
    gap: "2rem",
    textAlign: "center",
  },
  button: {
    alignItems: "center",
    borderRadius: radii.full,
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "flex",
    fontSize: "1rem",
    fontWeight: 500,
    gap: ".5rem",
    height: "3rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.7,
    },
    pointerEvents: {
      default: "auto",
      ":disabled": "none",
    },
    scale: {
      default: 1,
      ":hover": 1.02,
      ":active": 0.98,
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  buttonPrimary: {
    backgroundImage: `linear-gradient(to top, ${colors.primary}, ${colors.mutedForeground})`,
    boxShadow: {
      default: shadows.sm,
      ":hover": shadows.lg,
    },
    color: colors.primaryForeground,
  },
  buttonDanger: {
    backgroundImage: `linear-gradient(to top, ${colors.destructive}, ${colors.alertForeground})`,
    boxShadow: {
      default: shadows.sm,
      ":hover": shadows.lg,
    },
    color: colors.destructiveForeground,
  },
  buttonSecondary: {
    backgroundImage: `linear-gradient(to bottom, ${colors.card}, ${colors.muted})`,
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":hover": shadows.lg,
    },
    color: colors.mutedForeground,
  },
});

const buttonVariantStyles = {
  danger: styles.buttonDanger,
  primary: styles.buttonPrimary,
  secondary: styles.buttonSecondary,
};

export function integrationButtonStyles(
  variant: keyof typeof buttonVariantStyles,
) {
  return [styles.button, buttonVariantStyles[variant]];
}
export function IntegrationButton({
  variant = "primary",
  sx,
  style,
  ...props
}: {
  variant?: keyof typeof buttonVariantStyles;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className"> &
  StyleXProps) {
  return (
    <button
      {...props}
      {...mergeStyleXProps(
        [integrationButtonStyles(variant), sx],
        undefined,
        style,
      )}
    />
  );
}
export function IntegrationPageLayout({ children }: { children: ReactNode }) {
  return (
    <div {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>{children}</div>
    </div>
  );
}
