import { type ComponentProps, type ReactNode } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

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
  className,
}: {
  icon?: ReactNode;
  children: ReactNode;
  onClick?: () => void;
  onMouseEnter?: () => void;
  onMouseLeave?: () => void;
  disabled?: boolean;
  error?: boolean;
  subtle?: boolean;
  className?: string;
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
      className={cn([
        "rounded-full border-2 transition-[border-color,opacity] duration-200 focus-visible:ring-0",
        error && "border-red-500",
        !error && "border-neutral-200",
        subtle && "opacity-40 hover:opacity-100",
        className,
      ])}
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
        className="rounded-xl pr-1.5"
      >
        {tooltip.content}
      </TooltipContent>
    </Tooltip>
  );
}
