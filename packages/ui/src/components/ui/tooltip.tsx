"use client";

import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import { motion } from "motion/react";
import * as React from "react";

import { cn } from "@hypr/utils";

const TooltipProvider = TooltipPrimitive.Provider;

const Tooltip = TooltipPrimitive.Root;

const TooltipTrigger = TooltipPrimitive.Trigger;

const TooltipContent = React.forwardRef<
  React.ComponentRef<typeof TooltipPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TooltipPrimitive.Content>
>(({ className, sideOffset = 4, side = "top", ...props }, ref) => {
  // Determine animation direction based on tooltip side
  const getInitialPosition = () => {
    switch (side) {
      case "top":
        return { y: 10 }; // Sprout upward from button
      case "bottom":
        return { y: -10 }; // Sprout downward from button
      case "left":
        return { x: 10 }; // Sprout leftward from button
      case "right":
        return { x: -10 }; // Sprout rightward from button
      default:
        return { y: 10 };
    }
  };

  const initialPosition = getInitialPosition();

  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        ref={ref}
        sideOffset={sideOffset}
        side={side}
        asChild
        {...props}
      >
        <motion.div
          initial={{ opacity: 0, scale: 0.95, ...initialPosition }}
          animate={{ opacity: 1, scale: 1, x: 0, y: 0 }}
          exit={{ opacity: 0, scale: 0.95, ...initialPosition }}
          transition={{
            duration: 0.2,
            ease: [0.16, 1, 0.3, 1],
          }}
          className={cn([
            "z-50 overflow-hidden rounded-md px-3 py-1.5 text-xs",
            "border border-neutral-200/50 bg-white/80 text-neutral-700 shadow-lg backdrop-blur-sm",
            "origin-(--radix-tooltip-content-transform-origin)",
            className,
          ])}
        >
          {props.children}
        </motion.div>
      </TooltipPrimitive.Content>
    </TooltipPrimitive.Portal>
  );
});
TooltipContent.displayName = TooltipPrimitive.Content.displayName;

export { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger };
