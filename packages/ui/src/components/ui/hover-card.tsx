import * as HoverCardPrimitive from "@radix-ui/react-hover-card";
import { motion, type MotionStyle } from "motion/react";
import * as React from "react";

import { cn } from "@hypr/utils";

const HoverCard = HoverCardPrimitive.Root;
const HoverCardTrigger = HoverCardPrimitive.Trigger;

const HoverCardContent = React.forwardRef<
  React.ComponentRef<typeof HoverCardPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof HoverCardPrimitive.Content> & {
    followStyle?: MotionStyle;
  }
>(
  (
    {
      className,
      align = "center",
      sideOffset = 4,
      side = "bottom",
      followStyle,
      ...props
    },
    ref,
  ) => {
    const getInitialPosition = () => {
      switch (side) {
        case "top":
          return { y: 6 };
        case "bottom":
          return { y: -6 };
        case "left":
          return { x: 6 };
        case "right":
          return { x: -6 };
        default:
          return { y: -6 };
      }
    };

    const initialPosition = getInitialPosition();

    return (
      <HoverCardPrimitive.Portal>
        <HoverCardPrimitive.Content
          ref={ref}
          align={align}
          sideOffset={sideOffset}
          side={side}
          asChild
          {...props}
        >
          <motion.div
            initial={{
              opacity: 0,
              scale: 0.97,
              filter: "blur(2px)",
              ...initialPosition,
            }}
            animate={{
              opacity: 1,
              scale: 1,
              filter: "blur(0px)",
              x: 0,
              y: 0,
              transition: {
                duration: 0.2,
                ease: [0.16, 1, 0.3, 1],
              },
            }}
            exit={{
              opacity: 0,
              scale: 0.97,
              filter: "blur(2px)",
              ...initialPosition,
              transition: {
                duration: 0.1,
                ease: "easeIn",
              },
            }}
            style={followStyle}
            className={cn([
              "bg-popover text-popover-foreground z-50 w-64 rounded-md border p-4 shadow-md outline-hidden",
              "origin-(--radix-hover-card-content-transform-origin)",
              className,
            ])}
          >
            {props.children}
          </motion.div>
        </HoverCardPrimitive.Content>
      </HoverCardPrimitive.Portal>
    );
  },
);
HoverCardContent.displayName = HoverCardPrimitive.Content.displayName;

export { HoverCard, HoverCardContent, HoverCardTrigger };
