import { CaretDown, CaretUp, Check } from "@phosphor-icons/react";
import * as SelectPrimitive from "@radix-ui/react-select";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import { floatingContentStyle } from "./floating-content";

const Select = SelectPrimitive.Root;
const SelectGroup = SelectPrimitive.Group;

const SelectValue = React.forwardRef<
  React.ComponentRef<typeof SelectPrimitive.Value>,
  React.ComponentPropsWithoutRef<typeof SelectPrimitive.Value> & StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <SelectPrimitive.Value
    {...props}
    {...mergeStyleXProps([styles.value, sx], className, style)}
    ref={ref}
  />
));
SelectValue.displayName = SelectPrimitive.Value.displayName;

const SelectTrigger = React.forwardRef<
  React.ComponentRef<typeof SelectPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof SelectPrimitive.Trigger> & StyleXProps
>(({ className, children, style, sx, ...props }, ref) => (
  <SelectPrimitive.Trigger
    {...props}
    {...mergeStyleXProps(
      [styles.trigger, styles.focusOutlineHidden, sx],
      className,
      style,
    )}
    ref={ref}
  >
    {children}
    <SelectPrimitive.Icon asChild>
      <CaretDown {...stylex.props(styles.triggerIcon)} />
    </SelectPrimitive.Icon>
  </SelectPrimitive.Trigger>
));
SelectTrigger.displayName = SelectPrimitive.Trigger.displayName;

const SelectScrollUpButton = React.forwardRef<
  React.ComponentRef<typeof SelectPrimitive.ScrollUpButton>,
  React.ComponentPropsWithoutRef<typeof SelectPrimitive.ScrollUpButton> &
    StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <SelectPrimitive.ScrollUpButton
    {...props}
    {...mergeStyleXProps([styles.scrollButton, sx], className, style)}
    ref={ref}
  >
    <CaretUp {...stylex.props(styles.icon)} />
  </SelectPrimitive.ScrollUpButton>
));
SelectScrollUpButton.displayName = SelectPrimitive.ScrollUpButton.displayName;

const SelectScrollDownButton = React.forwardRef<
  React.ComponentRef<typeof SelectPrimitive.ScrollDownButton>,
  React.ComponentPropsWithoutRef<typeof SelectPrimitive.ScrollDownButton> &
    StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <SelectPrimitive.ScrollDownButton
    {...props}
    {...mergeStyleXProps([styles.scrollButton, sx], className, style)}
    ref={ref}
  >
    <CaretDown {...stylex.props(styles.icon)} />
  </SelectPrimitive.ScrollDownButton>
));
SelectScrollDownButton.displayName =
  SelectPrimitive.ScrollDownButton.displayName;

const SelectContent = React.forwardRef<
  React.ComponentRef<typeof SelectPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof SelectPrimitive.Content> & StyleXProps
>(({ className, children, position = "popper", style, sx, ...props }, ref) => (
  <SelectPrimitive.Portal>
    <SelectPrimitive.Content
      {...props}
      {...mergeStyleXProps(
        [
          floatingContentStyle,
          styles.content,
          position === "popper" && styles.popperContent,
          sx,
        ],
        className,
        style,
      )}
      ref={ref}
      position={position}
    >
      <SelectScrollUpButton />
      <SelectPrimitive.Viewport
        {...stylex.props(
          styles.viewport,
          position === "popper" && styles.popperViewport,
        )}
      >
        {children}
      </SelectPrimitive.Viewport>
      <SelectScrollDownButton />
    </SelectPrimitive.Content>
  </SelectPrimitive.Portal>
));
SelectContent.displayName = SelectPrimitive.Content.displayName;

const SelectLabel = React.forwardRef<
  React.ComponentRef<typeof SelectPrimitive.Label>,
  React.ComponentPropsWithoutRef<typeof SelectPrimitive.Label> & StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <SelectPrimitive.Label
    {...props}
    {...mergeStyleXProps([styles.label, sx], className, style)}
    ref={ref}
  />
));
SelectLabel.displayName = SelectPrimitive.Label.displayName;

const SelectItem = React.forwardRef<
  React.ComponentRef<typeof SelectPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof SelectPrimitive.Item> & StyleXProps
>(({ className, children, style, sx, ...props }, ref) => (
  <SelectPrimitive.Item
    {...props}
    {...mergeStyleXProps(
      [styles.item, styles.outlineHidden, sx],
      className,
      style,
    )}
    ref={ref}
  >
    <span {...stylex.props(styles.itemIndicator)}>
      <SelectPrimitive.ItemIndicator>
        <Check {...stylex.props(styles.icon)} />
      </SelectPrimitive.ItemIndicator>
    </span>
    <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
  </SelectPrimitive.Item>
));
SelectItem.displayName = SelectPrimitive.Item.displayName;

const SelectSeparator = React.forwardRef<
  React.ComponentRef<typeof SelectPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof SelectPrimitive.Separator> & StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <SelectPrimitive.Separator
    {...props}
    {...mergeStyleXProps([styles.separator, sx], className, style)}
    ref={ref}
  />
));
SelectSeparator.displayName = SelectPrimitive.Separator.displayName;

const styles = stylex.create({
  content: {
    backgroundColor: colors.popover,
    borderColor: colors.border,
    borderRadius: "18px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow:
      "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    maxHeight: "var(--radix-select-content-available-height)",
    minWidth: "8rem",
    overflowX: "hidden",
    overflowY: "auto",
    position: "relative",
    transformOrigin: "var(--radix-select-content-transform-origin)",
    zIndex: 50,
  },
  focusOutlineHidden: {
    outlineColor: {
      default: null,
      ":focus": {
        default: null,
        "@media (forced-colors: active)": "transparent",
      },
    },
    outlineOffset: {
      default: null,
      ":focus": {
        default: null,
        "@media (forced-colors: active)": "2px",
      },
    },
    outlineStyle: {
      default: null,
      ":focus": {
        default: "none",
        "@media (forced-colors: active)": "solid",
      },
    },
    outlineWidth: {
      default: null,
      ":focus": {
        default: null,
        "@media (forced-colors: active)": "2px",
      },
    },
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  item: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ":focus": colors.accent,
      ":is([data-highlighted])": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: null,
      ":focus": colors.accentForeground,
      ":is([data-highlighted])": colors.accentForeground,
    },
    cursor: "default",
    display: "flex",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    opacity: {
      default: 1,
      ":is([data-disabled])": 0.5,
    },
    paddingBlock: "0.375rem",
    paddingLeft: "0.5rem",
    paddingRight: "2rem",
    pointerEvents: {
      default: null,
      ":is([data-disabled])": "none",
    },
    position: "relative",
    userSelect: "none",
    width: "100%",
  },
  outlineHidden: {
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
  },
  itemIndicator: {
    alignItems: "center",
    display: "flex",
    height: "0.875rem",
    justifyContent: "center",
    position: "absolute",
    right: "0.5rem",
    width: "0.875rem",
  },
  label: {
    fontSize: "0.875rem",
    fontWeight: 600,
    lineHeight: "1.25rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
  },
  popperContent: {
    translate: {
      default: null,
      ':is([data-side="bottom"])': "0 0.25rem",
      ':is([data-side="left"])': "-0.25rem 0",
      ':is([data-side="right"])': "0.25rem 0",
      ':is([data-side="top"])': "0 -0.25rem",
    },
  },
  popperViewport: {
    height: "var(--radix-select-trigger-height)",
    minWidth: "var(--radix-select-trigger-width)",
    width: "100%",
  },
  scrollButton: {
    alignItems: "center",
    cursor: "default",
    display: "flex",
    justifyContent: "center",
    paddingBlock: "0.25rem",
  },
  separator: {
    backgroundColor: colors.muted,
    height: "1px",
    marginBlock: "0.25rem",
    marginInline: "-0.25rem",
  },
  trigger: {
    alignItems: "center",
    backgroundColor: "transparent",
    borderColor: colors.input,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: shadows.sm,
      ":focus": `0 0 0 1px ${colors.ring}, ${shadows.sm}`,
    },
    color: {
      default: null,
      ":is([data-placeholder])": colors.mutedForeground,
    },
    cursor: {
      default: null,
      ":disabled": "not-allowed",
    },
    display: "flex",
    fontSize: "0.875rem",
    height: "2.25rem",
    justifyContent: "space-between",
    lineHeight: "1.25rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    whiteSpace: "nowrap",
    width: "100%",
  },
  triggerIcon: {
    flexShrink: 0,
    height: "1rem",
    marginRight: "-0.25rem",
    opacity: 0.5,
    width: "1rem",
  },
  value: {
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: 1,
    display: "-webkit-box",
    overflow: "hidden",
  },
  viewport: {
    padding: "0.25rem",
  },
});

export {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectScrollDownButton,
  SelectScrollUpButton,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
};
