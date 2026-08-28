import { CaretRight, Check, Circle } from "@phosphor-icons/react";
import * as DropdownMenuPrimitive from "@radix-ui/react-dropdown-menu";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import {
  AppFloatingPanel,
  appFloatingContentStyle,
  floatingContentStyle,
  type FloatingContentVariant,
} from "./floating-content";

const DropdownMenu = DropdownMenuPrimitive.Root;
const DropdownMenuTrigger = DropdownMenuPrimitive.Trigger;
const DropdownMenuGroup = DropdownMenuPrimitive.Group;
const DropdownMenuPortal = DropdownMenuPrimitive.Portal;
const DropdownMenuSub = DropdownMenuPrimitive.Sub;
const DropdownMenuRadioGroup = DropdownMenuPrimitive.RadioGroup;

const DropdownMenuSubTrigger = React.forwardRef<
  React.ComponentRef<typeof DropdownMenuPrimitive.SubTrigger>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.SubTrigger> & {
    inset?: boolean;
  } & StyleXProps
>(({ className, inset, children, style, sx, ...props }, ref) => (
  <DropdownMenuPrimitive.SubTrigger
    {...props}
    {...mergeStyleXProps(
      [
        styles.subTrigger,
        styles.descendantIcons,
        styles.outlineHidden,
        inset && styles.inset,
        sx,
      ],
      className,
      style,
    )}
    ref={ref}
  >
    {children}
    <CaretRight {...stylex.props(styles.subTriggerCaret)} />
  </DropdownMenuPrimitive.SubTrigger>
));
DropdownMenuSubTrigger.displayName =
  DropdownMenuPrimitive.SubTrigger.displayName;

const DropdownMenuSubContent = React.forwardRef<
  React.ComponentRef<typeof DropdownMenuPrimitive.SubContent>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.SubContent> & {
    variant?: FloatingContentVariant;
  } & StyleXProps
>(({ className, style, sx, variant = "default", ...props }, ref) => (
  <DropdownMenuPrimitive.SubContent
    {...props}
    {...mergeStyleXProps(
      [
        floatingContentStyle,
        styles.content,
        variant === "app" ? appFloatingContentStyle : styles.defaultSubContent,
        sx,
      ],
      className,
      style,
    )}
    ref={ref}
  />
));
DropdownMenuSubContent.displayName =
  DropdownMenuPrimitive.SubContent.displayName;

const DropdownMenuContent = React.forwardRef<
  React.ComponentRef<typeof DropdownMenuPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Content> & {
    variant?: FloatingContentVariant;
  } & StyleXProps
>(
  (
    { className, sideOffset = 4, style, sx, variant = "default", ...props },
    ref,
  ) => (
    <DropdownMenuPrimitive.Portal>
      <DropdownMenuPrimitive.Content
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
        ref={ref}
        sideOffset={sideOffset}
      />
    </DropdownMenuPrimitive.Portal>
  ),
);
DropdownMenuContent.displayName = DropdownMenuPrimitive.Content.displayName;

const DropdownMenuItem = React.forwardRef<
  React.ComponentRef<typeof DropdownMenuPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Item> & {
    inset?: boolean;
  } & StyleXProps
>(({ className, inset, style, sx, ...props }, ref) => (
  <DropdownMenuPrimitive.Item
    {...props}
    {...mergeStyleXProps(
      [
        styles.item,
        styles.descendantIcons,
        styles.outlineHidden,
        inset && styles.inset,
        sx,
      ],
      className,
      style,
    )}
    ref={ref}
  />
));
DropdownMenuItem.displayName = DropdownMenuPrimitive.Item.displayName;

const DropdownMenuCheckboxItem = React.forwardRef<
  React.ComponentRef<typeof DropdownMenuPrimitive.CheckboxItem>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.CheckboxItem> &
    StyleXProps
>(({ className, children, checked, style, sx, ...props }, ref) => (
  <DropdownMenuPrimitive.CheckboxItem
    {...props}
    {...mergeStyleXProps(
      [styles.choiceItem, styles.outlineHidden, sx],
      className,
      style,
    )}
    ref={ref}
    checked={checked}
  >
    <span {...stylex.props(styles.leftIndicator)}>
      <DropdownMenuPrimitive.ItemIndicator>
        <Check {...stylex.props(styles.icon)} />
      </DropdownMenuPrimitive.ItemIndicator>
    </span>
    {children}
  </DropdownMenuPrimitive.CheckboxItem>
));
DropdownMenuCheckboxItem.displayName =
  DropdownMenuPrimitive.CheckboxItem.displayName;

const DropdownMenuRadioItem = React.forwardRef<
  React.ComponentRef<typeof DropdownMenuPrimitive.RadioItem>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.RadioItem> &
    StyleXProps
>(({ className, children, style, sx, ...props }, ref) => (
  <DropdownMenuPrimitive.RadioItem
    {...props}
    {...mergeStyleXProps(
      [styles.choiceItem, styles.outlineHidden, sx],
      className,
      style,
    )}
    ref={ref}
  >
    <span {...stylex.props(styles.leftIndicator)}>
      <DropdownMenuPrimitive.ItemIndicator>
        <Circle {...stylex.props(styles.radioIcon)} weight="fill" />
      </DropdownMenuPrimitive.ItemIndicator>
    </span>
    {children}
  </DropdownMenuPrimitive.RadioItem>
));
DropdownMenuRadioItem.displayName = DropdownMenuPrimitive.RadioItem.displayName;

const DropdownMenuLabel = React.forwardRef<
  React.ComponentRef<typeof DropdownMenuPrimitive.Label>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Label> & {
    inset?: boolean;
  } & StyleXProps
>(({ className, inset, style, sx, ...props }, ref) => (
  <DropdownMenuPrimitive.Label
    {...props}
    {...mergeStyleXProps(
      [styles.label, inset && styles.inset, sx],
      className,
      style,
    )}
    ref={ref}
  />
));
DropdownMenuLabel.displayName = DropdownMenuPrimitive.Label.displayName;

const DropdownMenuSeparator = React.forwardRef<
  React.ComponentRef<typeof DropdownMenuPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Separator> &
    StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <DropdownMenuPrimitive.Separator
    {...props}
    {...mergeStyleXProps([styles.separator, sx], className, style)}
    ref={ref}
  />
));
DropdownMenuSeparator.displayName = DropdownMenuPrimitive.Separator.displayName;

const DropdownMenuShortcut = ({
  className,
  style,
  sx,
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & StyleXProps) => {
  return (
    <span
      {...props}
      {...mergeStyleXProps([styles.shortcut, sx], className, style)}
    />
  );
};
DropdownMenuShortcut.displayName = "DropdownMenuShortcut";

const styles = stylex.create({
  choiceItem: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ":focus": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: null,
      ":focus": colors.accentForeground,
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
    paddingLeft: "2rem",
    paddingRight: "0.5rem",
    pointerEvents: {
      default: null,
      ":is([data-disabled])": "none",
    },
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  content: {
    minWidth: "8rem",
    overflow: "hidden",
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
    padding: "0.25rem",
  },
  defaultSubContent: {
    backgroundColor: colors.popover,
    borderColor: colors.border,
    borderRadius: "18px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.lg,
    padding: "0.25rem",
  },
  descendantIcons: {
    flexShrink: {
      default: null,
      ":is(*) svg": 0,
    },
    height: {
      default: null,
      ":is(*) svg": "1rem",
    },
    pointerEvents: {
      default: null,
      ":is(*) svg": "none",
    },
    width: {
      default: null,
      ":is(*) svg": "1rem",
    },
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  inset: {
    paddingLeft: "2rem",
  },
  item: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ":focus": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: null,
      ":focus": colors.accentForeground,
    },
    cursor: "default",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
    opacity: {
      default: 1,
      ":is([data-disabled])": 0.5,
    },
    paddingBlock: "0.375rem",
    paddingLeft: "0.5rem",
    paddingRight: "0.5rem",
    pointerEvents: {
      default: null,
      ":is([data-disabled])": "none",
    },
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
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
  label: {
    fontSize: "0.875rem",
    fontWeight: 600,
    lineHeight: "1.25rem",
    paddingBlock: "0.375rem",
    paddingLeft: "0.5rem",
    paddingRight: "0.5rem",
  },
  leftIndicator: {
    alignItems: "center",
    display: "flex",
    height: "0.875rem",
    justifyContent: "center",
    left: "0.5rem",
    position: "absolute",
    width: "0.875rem",
  },
  radioIcon: {
    height: "0.5rem",
    width: "0.5rem",
  },
  separator: {
    backgroundColor: colors.muted,
    height: "1px",
    marginBlock: "0.25rem",
    marginInline: "-0.25rem",
  },
  shortcut: {
    fontSize: "0.75rem",
    letterSpacing: "0.1em",
    lineHeight: "1rem",
    marginLeft: "auto",
    opacity: 0.6,
  },
  subTrigger: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ":focus": colors.accent,
      ':is([data-state="open"])': colors.accent,
    },
    borderRadius: radii.full,
    cursor: "default",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
    paddingBlock: "0.375rem",
    paddingLeft: "0.5rem",
    paddingRight: "0.5rem",
  },
  subTriggerCaret: {
    marginLeft: "auto",
  },
});

export {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
};
