"use client";

import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import * as stylex from "@stylexjs/stylex";
import { motion } from "motion/react";
import * as React from "react";

import { radii, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const TooltipProvider = TooltipPrimitive.Provider;

const Tooltip = TooltipPrimitive.Root;

const TooltipTrigger = TooltipPrimitive.Trigger;

const TooltipContent = React.forwardRef<
  React.ComponentRef<typeof TooltipPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TooltipPrimitive.Content> & StyleXProps
>(
  (
    { children, className, sideOffset = 4, side = "top", style, sx, ...props },
    ref,
  ) => {
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
            {...mergeStyleXProps([styles.content, sx], className, style)}
          >
            {children}
          </motion.div>
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    );
  },
);
TooltipContent.displayName = TooltipPrimitive.Content.displayName;

const styles = stylex.create({
  content: {
    backdropFilter: "blur(4px)",
    backgroundColor: "rgb(255 255 255 / 0.8)",
    borderColor: "rgb(229 229 229 / 0.5)",
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.lg,
    color: "oklch(37.1% 0 0)",
    fontSize: "0.75rem",
    lineHeight: "1rem",
    overflow: "hidden",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    transformOrigin: "var(--radix-tooltip-content-transform-origin)",
    zIndex: 50,
  },
});

export { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger };
