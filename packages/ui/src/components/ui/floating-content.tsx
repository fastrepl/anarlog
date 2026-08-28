import * as stylex from "@stylexjs/stylex";

import { colors, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

export type FloatingContentVariant = "default" | "app";

export function AppFloatingPanel({
  className,
  style,
  sx,
  ...props
}: React.HTMLAttributes<HTMLDivElement> & StyleXProps) {
  return (
    <div
      {...props}
      {...mergeStyleXProps([styles.panel, sx], className, style)}
    />
  );
}

const floatingContentEnter = stylex.keyframes({
  from: {
    opacity: 0,
    transform: "scale3d(0.95, 0.95, 0.95)",
  },
});

const floatingContentEnterFromTop = stylex.keyframes({
  from: {
    opacity: 0,
    transform: "translate3d(0, -0.5rem, 0) scale3d(0.95, 0.95, 0.95)",
  },
});

const floatingContentEnterFromLeft = stylex.keyframes({
  from: {
    opacity: 0,
    transform: "translate3d(-0.5rem, 0, 0) scale3d(0.95, 0.95, 0.95)",
  },
});

const floatingContentEnterFromRight = stylex.keyframes({
  from: {
    opacity: 0,
    transform: "translate3d(0.5rem, 0, 0) scale3d(0.95, 0.95, 0.95)",
  },
});

const floatingContentEnterFromBottom = stylex.keyframes({
  from: {
    opacity: 0,
    transform: "translate3d(0, 0.5rem, 0) scale3d(0.95, 0.95, 0.95)",
  },
});

const floatingContentExit = stylex.keyframes({
  to: {
    opacity: 0,
    transform: "scale3d(0.95, 0.95, 0.95)",
  },
});

const styles = stylex.create({
  appContent: {
    backgroundColor: colors.appFloatingChrome,
    borderColor: colors.appFloatingBorder,
    borderRadius: "22px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.lg,
    color: colors.popoverForeground,
    overflow: "hidden",
    padding: "0.125rem",
  },
  floatingContent: {
    animationDuration: {
      default: null,
      ':is([data-state="closed"])': "150ms",
      ':is([data-state="open"])': "150ms",
    },
    animationName: {
      default: null,
      ':is([data-state="closed"])': floatingContentExit,
      ':is([data-state="open"])': floatingContentEnter,
      ':is([data-state="open"][data-side="bottom"])':
        floatingContentEnterFromTop,
      ':is([data-state="open"][data-side="left"])':
        floatingContentEnterFromRight,
      ':is([data-state="open"][data-side="right"])':
        floatingContentEnterFromLeft,
      ':is([data-state="open"][data-side="top"])':
        floatingContentEnterFromBottom,
    },
    color: colors.popoverForeground,
  },
  panel: {
    backgroundColor: colors.appFloatingPanel,
    borderColor: colors.appFloatingBorder,
    borderRadius: "18px",
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.popoverForeground,
  },
});

export const appFloatingContentStyle = styles.appContent;
export const floatingContentStyle = styles.floatingContent;
export const appFloatingContentClassName =
  stylex.props(appFloatingContentStyle).className ?? "";
