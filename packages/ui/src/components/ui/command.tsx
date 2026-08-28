import { MagnifyingGlass } from "@phosphor-icons/react";
import { type DialogProps } from "@radix-ui/react-dialog";
import * as stylex from "@stylexjs/stylex";
import { Command as CommandPrimitive } from "cmdk";
import * as React from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Dialog, DialogContent } from "@anlg/ui/components/ui/dialog";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

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
        <Command sx={styles.dialogCommand}>{children}</Command>
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
  const resolvedStyle = mergeStyleXProps([styles.group, sx], className, style);

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
  const resolvedStyle = mergeStyleXProps([styles.item, sx], className, style);

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
  dialogCommand: {
    color: {
      default: null,
      ":is(*) [cmdk-group-heading]": colors.mutedForeground,
    },
    fontWeight: {
      default: null,
      ":is(*) [cmdk-group-heading]": 500,
    },
    height: {
      default: null,
      ":is(*) [cmdk-input]": "3rem",
      ":is(*) [cmdk-input-wrapper] svg": "1.25rem",
      ":is(*) [cmdk-item] svg": "1.25rem",
    },
    paddingBlock: {
      default: null,
      ":is(*) [cmdk-item]": "0.75rem",
    },
    paddingInline: {
      default: null,
      ":is(*) [cmdk-group]": "0.5rem",
      ":is(*) [cmdk-group-heading]": "0.5rem",
      ":is(*) [cmdk-item]": "0.5rem",
    },
    paddingTop: {
      default: null,
      ":is(*) [cmdk-group]:not([hidden]) ~ [cmdk-group]": 0,
    },
    width: {
      default: null,
      ":is(*) [cmdk-input-wrapper] svg": "1.25rem",
      ":is(*) [cmdk-item] svg": "1.25rem",
    },
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
    fontSize: {
      default: null,
      ":is(*) [cmdk-group-heading]": "0.75rem",
    },
    fontWeight: {
      default: null,
      ":is(*) [cmdk-group-heading]": 500,
    },
    overflow: "hidden",
    paddingBlock: {
      default: "0.25rem",
      ":is(*) [cmdk-group-heading]": "0.375rem",
    },
    paddingInline: {
      default: "0.25rem",
      ":is(*) [cmdk-group-heading]": "0.5rem",
    },
  },
  separator: {
    backgroundColor: colors.border,
    height: "1px",
    marginInline: "-0.25rem",
  },
  item: {
    alignItems: "center",
    backgroundColor: {
      default: null,
      ':is([data-selected="true"])': colors.accent,
    },
    borderRadius: radii.full,
    cursor: "default",
    display: "flex",
    fontSize: "0.875rem",
    flexShrink: {
      default: null,
      ":is(*) svg": 0,
    },
    gap: "0.5rem",
    height: {
      default: null,
      ":is(*) svg": "1rem",
    },
    lineHeight: "1.25rem",
    opacity: {
      default: 1,
      ':is([data-disabled="true"])': 0.5,
    },
    outlineColor: "transparent",
    outlineOffset: "2px",
    outlineStyle: "solid",
    outlineWidth: "2px",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
    pointerEvents: {
      default: null,
      ':is([data-disabled="true"])': "none",
      ":is(*) svg": "none",
    },
    position: "relative",
    userSelect: "none",
    width: {
      default: null,
      ":is(*) svg": "1rem",
    },
    color: {
      default: null,
      ':is([data-selected="true"])': colors.accentForeground,
    },
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
