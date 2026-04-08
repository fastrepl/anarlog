import * as PopoverPrimitive from "@radix-ui/react-popover";
import * as React from "react";

import { cn } from "@hypr/utils";

import {
  AppFloatingPanel,
  appFloatingContentClassName,
  type FloatingContentVariant,
} from "./floating-content";

const Popover = PopoverPrimitive.Root;
const PopoverTrigger = PopoverPrimitive.Trigger;
const PopoverAnchor = PopoverPrimitive.Anchor;

const PopoverContent = React.forwardRef<
  React.ComponentRef<typeof PopoverPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof PopoverPrimitive.Content> & {
    variant?: FloatingContentVariant;
  }
>(
  (
    {
      className,
      align = "center",
      sideOffset = 4,
      variant = "default",
      ...props
    },
    ref,
  ) => (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        ref={ref}
        align={align}
        sideOffset={sideOffset}
        className={cn([
          "text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 w-72 origin-(--radix-popover-content-transform-origin) outline-hidden",
          variant === "app"
            ? appFloatingContentClassName
            : "bg-popover rounded-md border p-4 shadow-md",
          className,
        ])}
        {...props}
      />
    </PopoverPrimitive.Portal>
  ),
);
PopoverContent.displayName = PopoverPrimitive.Content.displayName;

export {
  AppFloatingPanel,
  Popover,
  PopoverAnchor,
  PopoverContent,
  PopoverTrigger,
};
