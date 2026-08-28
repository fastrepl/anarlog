import * as stylex from "@stylexjs/stylex";
import { forwardRef } from "react";

import {
  colors,
  radii,
  shadows,
  spacing,
} from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

interface CardProps extends React.HTMLAttributes<HTMLDivElement>, StyleXProps {
  variant?: "default" | "outline-solid" | "ghost";
}

export const Card = forwardRef<HTMLDivElement, CardProps>(
  ({ className, style, sx, variant = "default", ...props }, ref) => (
    <div
      {...props}
      {...mergeStyleXProps(
        [
          styles.card,
          variant === "default" && styles.cardDefault,
          variant === "outline-solid" && styles.cardOutline,
          variant === "ghost" && styles.cardGhost,
          sx,
        ],
        className,
        style,
      )}
      ref={ref}
    />
  ),
);
Card.displayName = "Card";

interface CardHeaderProps
  extends React.HTMLAttributes<HTMLDivElement>, StyleXProps {
  spacing?: "default" | "compact" | "loose";
}

export const CardHeader = forwardRef<HTMLDivElement, CardHeaderProps>(
  ({ className, style, sx, spacing = "default", ...props }, ref) => (
    <div
      {...props}
      {...mergeStyleXProps(
        [
          styles.cardHeader,
          spacing === "default" && styles.spacingDefault,
          spacing === "compact" && styles.spacingCompact,
          spacing === "loose" && styles.spacingLoose,
          sx,
        ],
        className,
        style,
      )}
      ref={ref}
    />
  ),
);
CardHeader.displayName = "CardHeader";

interface CardTitleProps
  extends React.HTMLAttributes<HTMLHeadingElement>, StyleXProps {
  as?: "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
}

export const CardTitle = forwardRef<HTMLHeadingElement, CardTitleProps>(
  ({ className, style, sx, as: Component = "h3", ...props }, ref) => (
    <Component
      {...props}
      {...mergeStyleXProps([styles.cardTitle, sx], className, style)}
      ref={ref}
    />
  ),
);
CardTitle.displayName = "CardTitle";

interface CardDescriptionProps
  extends React.HTMLAttributes<HTMLParagraphElement>, StyleXProps {}

export const CardDescription = forwardRef<
  HTMLParagraphElement,
  CardDescriptionProps
>(({ className, style, sx, ...props }, ref) => (
  <p
    {...props}
    {...mergeStyleXProps([styles.cardDescription, sx], className, style)}
    ref={ref}
  />
));
CardDescription.displayName = "CardDescription";

interface CardContentProps
  extends React.HTMLAttributes<HTMLDivElement>, StyleXProps {
  spacing?: "default" | "compact" | "loose";
}

export const CardContent = forwardRef<HTMLDivElement, CardContentProps>(
  ({ className, style, sx, spacing = "default", ...props }, ref) => (
    <div
      {...props}
      {...mergeStyleXProps(
        [
          spacing === "default" && styles.contentSpacingDefault,
          spacing === "compact" && styles.contentSpacingCompact,
          spacing === "loose" && styles.contentSpacingLoose,
          sx,
        ],
        className,
        style,
      )}
      ref={ref}
    />
  ),
);
CardContent.displayName = "CardContent";

interface CardFooterProps
  extends React.HTMLAttributes<HTMLDivElement>, StyleXProps {
  spacing?: "default" | "compact" | "loose";
  align?: "start" | "center" | "end" | "between" | "around" | "evenly";
}

export const CardFooter = forwardRef<HTMLDivElement, CardFooterProps>(
  (
    { className, style, sx, spacing = "default", align = "between", ...props },
    ref,
  ) => (
    <div
      {...props}
      {...mergeStyleXProps(
        [
          styles.cardFooter,
          spacing === "default" && styles.contentSpacingDefault,
          spacing === "compact" && styles.contentSpacingCompact,
          spacing === "loose" && styles.contentSpacingLoose,
          align === "start" && styles.alignStart,
          align === "center" && styles.alignCenter,
          align === "end" && styles.alignEnd,
          align === "between" && styles.alignBetween,
          align === "around" && styles.alignAround,
          align === "evenly" && styles.alignEvenly,
          sx,
        ],
        className,
        style,
      )}
      ref={ref}
    />
  ),
);
CardFooter.displayName = "CardFooter";

const styles = stylex.create({
  card: {
    borderRadius: radii.lg,
  },
  cardDefault: {
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
  },
  cardOutline: {
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
  },
  cardGhost: {
    borderStyle: "none",
    boxShadow: "none",
  },
  cardHeader: {
    display: "flex",
    flexDirection: "column",
  },
  spacingDefault: {
    gap: "0.375rem",
    paddingBlock: spacing.xl,
    paddingInline: spacing.xl,
  },
  spacingCompact: {
    gap: spacing.xs,
    paddingBlock: spacing.lg,
    paddingInline: spacing.lg,
  },
  spacingLoose: {
    gap: spacing.sm,
    paddingBlock: spacing.xxl,
    paddingInline: spacing.xxl,
  },
  cardTitle: {
    fontWeight: 600,
    letterSpacing: "-0.025em",
    lineHeight: 1,
  },
  cardDescription: {
    color: "oklch(55.6% 0 0)",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  contentSpacingDefault: {
    paddingBottom: spacing.xl,
    paddingLeft: spacing.xl,
    paddingRight: spacing.xl,
    paddingTop: 0,
  },
  contentSpacingCompact: {
    paddingBottom: spacing.lg,
    paddingLeft: spacing.lg,
    paddingRight: spacing.lg,
    paddingTop: 0,
  },
  contentSpacingLoose: {
    paddingBottom: spacing.xxl,
    paddingLeft: spacing.xxl,
    paddingRight: spacing.xxl,
    paddingTop: 0,
  },
  cardFooter: {
    display: "flex",
  },
  alignStart: {
    justifyContent: "flex-start",
  },
  alignCenter: {
    justifyContent: "center",
  },
  alignEnd: {
    justifyContent: "flex-end",
  },
  alignBetween: {
    justifyContent: "space-between",
  },
  alignAround: {
    justifyContent: "space-around",
  },
  alignEvenly: {
    justifyContent: "space-evenly",
  },
});
