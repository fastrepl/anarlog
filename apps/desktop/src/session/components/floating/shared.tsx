import * as stylex from "@stylexjs/stylex";
import { type ComponentProps, type ReactNode } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";
import type { StyleXProps } from "@anlg/ui/lib/stylex";

export { ActionableTooltipContent } from "~/session/components/shared";

export function FloatingButton({
  icon,
  children,
  onClick,
  onMouseEnter,
  onMouseLeave,
  disabled,
  tooltip,
  error,
  subtle,
  sx,
}: {
  icon?: ReactNode;
  children: ReactNode;
  onClick?: () => void;
  onMouseEnter?: () => void;
  onMouseLeave?: () => void;
  disabled?: boolean;
  error?: boolean;
  subtle?: boolean;
  sx?: StyleXProps["sx"];
  tooltip?: {
    content: ReactNode;
    side?: ComponentProps<typeof TooltipContent>["side"];
    align?: ComponentProps<typeof TooltipContent>["align"];
    delayDuration?: number;
  };
}) {
  const button = (
    <Button
      size="lg"
      sx={[styles.button, error && styles.error, subtle && styles.subtle, sx]}
      onClick={onClick}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      disabled={disabled}
    >
      {icon}
      {children}
    </Button>
  );

  if (!tooltip) {
    return button;
  }

  return (
    <Tooltip delayDuration={tooltip.delayDuration ?? 0}>
      <TooltipTrigger asChild>
        <div>{button}</div>
      </TooltipTrigger>
      <TooltipContent
        side={tooltip.side ?? "top"}
        align={tooltip.align}
        sx={styles.tooltip}
      >
        {tooltip.content}
      </TooltipContent>
    </Tooltip>
  );
}

const styles = stylex.create({
  button: {
    backgroundColor: {
      default: colors.foreground,
      ":hover": colors.foreground,
    },
    borderColor: "rgb(255 255 255 / 0.2)",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "2px",
    boxShadow:
      "inset 0 0 0 1px rgb(255 255 255 / 0.22), 0 10px 28px rgb(0 0 0 / 0.28)",
    color: colors.background,
    transitionDuration: "200ms",
    transitionProperty:
      "background-color, border-color, color, opacity, box-shadow",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  error: {
    borderColor: "#ef4444",
  },
  subtle: {
    opacity: {
      default: 0.4,
      ":hover": 1,
    },
  },
  tooltip: {
    borderRadius: radii.xl,
    paddingRight: "0.375rem",
  },
});
