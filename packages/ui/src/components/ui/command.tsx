import { MagnifyingGlass } from "@phosphor-icons/react";
import { type DialogProps } from "@radix-ui/react-dialog";
import * as stylex from "@stylexjs/stylex";
import { Command as CommandPrimitive } from "cmdk";
import * as React from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Dialog, DialogContent } from "@anlg/ui/components/ui/dialog";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";
import { cn } from "@anlg/utils";

const Command = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const resolvedStyle = mergeStyleXProps(
    [styles.command, sx],
    className,
    style,
  );

  return (
    <CommandPrimitive
      {...props}
      ref={ref}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
});
Command.displayName = CommandPrimitive.displayName;

const CommandDialog = ({
  children,
  sx,
  ...props
}: DialogProps & StyleXProps) => {
  return (
    <Dialog {...props}>
      <DialogContent sx={[styles.dialogContent, sx]}>
        <Command className={commandDialogSelectorClassName}>{children}</Command>
      </DialogContent>
    </Dialog>
  );
};

const CommandInput = React.forwardRef<
  HTMLInputElement,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Input> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const wrapperStyle = mergeStyleXProps(styles.inputWrapper);
  const iconStyle = mergeStyleXProps(styles.inputIcon);
  const inputStyle = mergeStyleXProps([styles.input, sx], className, style);

  return (
    <div
      className={wrapperStyle.className}
      style={wrapperStyle.style}
      cmdk-input-wrapper=""
    >
      <MagnifyingGlass
        className={iconStyle.className}
        style={iconStyle.style}
      />
      <CommandPrimitive.Input
        {...props}
        ref={ref}
        className={inputStyle.className}
        style={inputStyle.style}
      />
    </div>
  );
});

CommandInput.displayName = CommandPrimitive.Input.displayName;

const CommandList = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.List>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.List> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const resolvedStyle = mergeStyleXProps([styles.list, sx], className, style);

  return (
    <CommandPrimitive.List
      {...props}
      ref={ref}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
});

CommandList.displayName = CommandPrimitive.List.displayName;

const CommandEmpty = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.Empty>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Empty> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const resolvedStyle = mergeStyleXProps([styles.empty, sx], className, style);

  return (
    <CommandPrimitive.Empty
      {...props}
      ref={ref}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
});

CommandEmpty.displayName = CommandPrimitive.Empty.displayName;

const CommandGroup = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.Group>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Group> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const resolvedStyle = mergeStyleXProps(
    [styles.group, sx],
    cn([commandGroupSelectorClassName, className]),
    style,
  );

  return (
    <CommandPrimitive.Group
      {...props}
      ref={ref}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
});

CommandGroup.displayName = CommandPrimitive.Group.displayName;

const CommandSeparator = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Separator> &
    StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const resolvedStyle = mergeStyleXProps(
    [styles.separator, sx],
    className,
    style,
  );

  return (
    <CommandPrimitive.Separator
      {...props}
      ref={ref}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
});
CommandSeparator.displayName = CommandPrimitive.Separator.displayName;

const CommandItem = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Item> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const resolvedStyle = mergeStyleXProps(
    [styles.item, sx],
    cn([commandItemSelectorClassName, className]),
    style,
  );

  return (
    <CommandPrimitive.Item
      {...props}
      ref={ref}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
});

CommandItem.displayName = CommandPrimitive.Item.displayName;

const CommandShortcut = ({
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
CommandShortcut.displayName = "CommandShortcut";

const commandDialogSelectorClassName =
  "**:[[cmdk-group-heading]]:text-muted-foreground [&_[cmdk-group]:not([hidden])_~[cmdk-group]]:pt-0 [&_[cmdk-input-wrapper]_svg]:h-5 [&_[cmdk-input-wrapper]_svg]:w-5 [&_[cmdk-item]_svg]:h-5 [&_[cmdk-item]_svg]:w-5 **:[[cmdk-group-heading]]:px-2 **:[[cmdk-group-heading]]:font-medium **:[[cmdk-group]]:px-2 **:[[cmdk-input]]:h-12 **:[[cmdk-item]]:px-2 **:[[cmdk-item]]:py-3";

const commandGroupSelectorClassName =
  "**:[[cmdk-group-heading]]:text-muted-foreground **:[[cmdk-group-heading]]:px-2 **:[[cmdk-group-heading]]:py-1.5 **:[[cmdk-group-heading]]:text-xs **:[[cmdk-group-heading]]:font-medium";

const commandItemSelectorClassName =
  "data-[selected=true]:bg-accent data-[selected=true]:text-accent-foreground data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0";

const styles = stylex.create({
  command: {
    backgroundColor: colors.popover,
    borderRadius: "18px",
    color: colors.popoverForeground,
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
    width: "100%",
  },
  dialogContent: {
    overflow: "hidden",
    padding: 0,
  },
  inputWrapper: {
    alignItems: "center",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    paddingInline: "0.75rem",
  },
  inputIcon: {
    flexShrink: 0,
    height: "1rem",
    marginRight: "0.5rem",
    opacity: 0.5,
    width: "1rem",
  },
  input: {
    "::placeholder": {
      color: colors.mutedForeground,
    },
    backgroundColor: "transparent",
    borderRadius: radii.full,
    cursor: {
      default: null,
      ":disabled": "not-allowed",
    },
    display: "flex",
    fontSize: "0.875rem",
    height: "2.5rem",
    lineHeight: "1.25rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    outlineColor: "transparent",
    outlineOffset: "2px",
    outlineStyle: "solid",
    outlineWidth: "2px",
    paddingBlock: "0.75rem",
    width: "100%",
  },
  list: {
    maxHeight: "300px",
    overflowX: "hidden",
    overflowY: "auto",
  },
  empty: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "1.5rem",
    textAlign: "center",
  },
  group: {
    color: colors.foreground,
    overflow: "hidden",
    padding: "0.25rem",
  },
  separator: {
    backgroundColor: colors.border,
    height: "1px",
    marginInline: "-0.25rem",
  },
  item: {
    alignItems: "center",
    borderRadius: radii.full,
    cursor: "default",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
    outlineColor: "transparent",
    outlineOffset: "2px",
    outlineStyle: "solid",
    outlineWidth: "2px",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
    position: "relative",
    userSelect: "none",
  },
  shortcut: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    letterSpacing: "0.1em",
    lineHeight: "1rem",
    marginLeft: "auto",
  },
});

export {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
};
