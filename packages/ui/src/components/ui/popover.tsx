import * as PopoverPrimitive from "@radix-ui/react-popover";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import {
  AppFloatingPanel,
  appFloatingContentStyle,
  floatingContentStyle,
  type FloatingContentVariant,
} from "./floating-content";

const Popover = PopoverPrimitive.Root;
const PopoverTrigger = PopoverPrimitive.Trigger;
const PopoverAnchor = PopoverPrimitive.Anchor;

const PopoverContent = React.forwardRef<
  React.ComponentRef<typeof PopoverPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof PopoverPrimitive.Content> & {
    variant?: FloatingContentVariant;
  } & StyleXProps
>(
  (
    {
      className,
      align = "center",
      sideOffset = 4,
      collisionPadding = 8,
      variant = "default",
      style,
      sx,
      ...props
    },
    ref,
  ) => (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        ref={ref}
        align={align}
        sideOffset={sideOffset}
        collisionPadding={collisionPadding}
        {...props}
        {...mergeStyleXProps(
          [
            floatingContentStyle,
            styles.content,
            variant === "app" ? appFloatingContentStyle : styles.defaultContent,
            sx,
          ],
          className,
          style,
        )}
      />
    </PopoverPrimitive.Portal>
  ),
);
PopoverContent.displayName = PopoverPrimitive.Content.displayName;

const styles = stylex.create({
  content: {
    outlineColor: {
      default: null,
      "@media (forced-colors: active)": "transparent",
    },
    outlineOffset: {
      default: null,
      "@media (forced-colors: active)": "2px",
    },
    outlineStyle: {
      default: "none",
      "@media (forced-colors: active)": "solid",
    },
    outlineWidth: {
      default: null,
      "@media (forced-colors: active)": "2px",
    },
    transformOrigin: "var(--radix-popover-content-transform-origin)",
    width: "18rem",
    zIndex: 50,
  },
  defaultContent: {
    backgroundColor: colors.popover,
    borderColor: colors.border,
    borderRadius: "18px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow:
      "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    padding: "1rem",
  },
});

export {
  AppFloatingPanel,
  Popover,
  PopoverAnchor,
  PopoverContent,
  PopoverTrigger,
};
