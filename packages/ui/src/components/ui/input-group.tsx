import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";
import { cn } from "@anlg/utils";

import { Button } from "./button";
import { Input } from "./input";
import { Textarea } from "./textarea";

function InputGroup({
  className,
  style,
  sx,
  ...props
}: React.ComponentProps<"div"> & StyleXProps) {
  return (
    <div
      data-slot="input-group"
      role="group"
      {...props}
      {...mergeStyleXProps(
        [styles.group, sx],
        cn([inputGroupSelectorClassName, className]),
        style,
      )}
    />
  );
}

function InputGroupAddon({
  className,
  style,
  sx,
  align = "inline-start",
  ...props
}: React.ComponentProps<"div"> &
  StyleXProps & {
    align?: "inline-start" | "inline-end" | "block-start" | "block-end" | null;
  }) {
  const resolvedAlign = align ?? "inline-start";

  return (
    <div
      role="group"
      data-slot="input-group-addon"
      data-align={align}
      onClick={(e) => {
        if ((e.target as HTMLElement).closest("button")) {
          return;
        }
        e.currentTarget.parentElement?.querySelector("input")?.focus();
      }}
      {...props}
      {...mergeStyleXProps(
        [styles.addon, inputGroupAddonAlignStyles[resolvedAlign], sx],
        cn([
          inputGroupAddonSelectorClassName,
          inputGroupAddonAlignSelectorClassNames[resolvedAlign],
          className,
        ]),
        style,
      )}
    />
  );
}

function InputGroupButton({
  className,
  style,
  sx,
  type = "button",
  variant = "ghost",
  size = "xs",
  ...props
}: Omit<React.ComponentProps<typeof Button>, "size"> & {
  size?: "xs" | "sm" | "icon-xs" | "icon-sm" | null;
}) {
  const resolvedSize = size ?? "xs";

  return (
    <Button
      type={type}
      data-size={size}
      variant={variant}
      {...props}
      className={cn([
        inputGroupButtonSizeSelectorClassNames[resolvedSize],
        className,
      ])}
      style={style}
      sx={[styles.button, inputGroupButtonSizeStyles[resolvedSize], sx]}
    />
  );
}

function InputGroupText({
  className,
  style,
  sx,
  ...props
}: React.ComponentProps<"span"> & StyleXProps) {
  return (
    <span
      {...props}
      {...mergeStyleXProps(
        [styles.text, sx],
        cn([inputGroupTextSelectorClassName, className]),
        style,
      )}
    />
  );
}

function InputGroupInput({
  className,
  style,
  sx,
  ...props
}: React.ComponentProps<"input"> & StyleXProps) {
  return (
    <Input
      data-slot="input-group-control"
      {...props}
      className={className}
      style={style}
      sx={[styles.control, sx]}
    />
  );
}

function InputGroupTextarea({
  className,
  style,
  sx,
  ...props
}: React.ComponentProps<"textarea"> & StyleXProps) {
  return (
    <Textarea
      data-slot="input-group-control"
      {...props}
      className={className}
      style={style}
      sx={[styles.textarea, sx]}
    />
  );
}

const inputGroupSelectorClassName =
  "group/input-group dark:bg-input/30 has-[>textarea]:h-auto [&>input]:has-[>[data-align=inline-start]]:pl-2 [&>input]:has-[>[data-align=inline-end]]:pr-2 has-[>[data-align=block-start]]:h-auto has-[>[data-align=block-start]]:flex-col [&>input]:has-[>[data-align=block-start]]:pb-3 has-[>[data-align=block-end]]:h-auto has-[>[data-align=block-end]]:flex-col [&>input]:has-[>[data-align=block-end]]:pt-3 has-[[data-slot=input-group-control]:focus-visible]:ring-ring has-[[data-slot=input-group-control]:focus-visible]:ring-1 has-[[data-slot][aria-invalid=true]]:ring-destructive/20 has-[[data-slot][aria-invalid=true]]:border-destructive dark:has-[[data-slot][aria-invalid=true]]:ring-destructive/40";

const inputGroupAddonSelectorClassName =
  "group-data-[disabled=true]/input-group:opacity-50 [&>kbd]:rounded-[calc(var(--radius)-5px)] [&>svg:not([class*='size-'])]:size-4";

const inputGroupAddonAlignSelectorClassNames = {
  "block-end": "group-has-[>input]/input-group:pb-2.5 [.border-t]:pt-3",
  "block-start": "group-has-[>input]/input-group:pt-2.5 [.border-b]:pb-3",
  "inline-end": "has-[>button]:mr-[-0.4rem] has-[>kbd]:mr-[-0.35rem]",
  "inline-start": "has-[>button]:ml-[-0.45rem] has-[>kbd]:ml-[-0.35rem]",
};

const inputGroupButtonSizeSelectorClassNames = {
  "icon-sm": undefined,
  "icon-xs": undefined,
  sm: undefined,
  xs: "[&>svg:not([class*='size-'])]:size-3.5",
};

const inputGroupTextSelectorClassName =
  "[&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4";

const styles = stylex.create({
  group: {
    alignItems: "center",
    borderColor: colors.input,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 1px rgb(0 0 0 / 0.05)",
    display: "flex",
    height: "2.25rem",
    outlineColor: "transparent",
    outlineOffset: "2px",
    outlineStyle: "solid",
    outlineWidth: "2px",
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty: "color, box-shadow",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  addon: {
    alignItems: "center",
    color: colors.mutedForeground,
    cursor: "text",
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: "0.5rem",
    height: "auto",
    justifyContent: "center",
    lineHeight: "1.25rem",
    paddingBottom: "0.375rem",
    paddingTop: "0.375rem",
    userSelect: "none",
  },
  addonInlineStart: {
    order: -9999,
    paddingLeft: "0.75rem",
  },
  addonInlineEnd: {
    order: 9999,
    paddingRight: "0.75rem",
  },
  addonBlockStart: {
    justifyContent: "flex-start",
    order: -9999,
    paddingInline: "0.75rem",
    paddingTop: "0.75rem",
    width: "100%",
  },
  addonBlockEnd: {
    justifyContent: "flex-start",
    order: 9999,
    paddingBottom: "0.75rem",
    paddingInline: "0.75rem",
    width: "100%",
  },
  button: {
    alignItems: "center",
    boxShadow: {
      default: "none",
      ":focus-visible": `0 0 0 1px ${colors.ring}`,
    },
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
  },
  buttonXs: {
    borderRadius: "calc(var(--radius, 0.5rem) - 5px)",
    gap: "0.25rem",
    height: "1.5rem",
    paddingInline: "0.5rem",
  },
  buttonSm: {
    borderRadius: radii.md,
    gap: "0.375rem",
    height: "2rem",
    paddingInline: "0.625rem",
  },
  buttonIconXs: {
    borderRadius: "calc(var(--radius, 0.5rem) - 5px)",
    height: "1.5rem",
    paddingBlock: 0,
    paddingInline: 0,
    width: "1.5rem",
  },
  buttonIconSm: {
    height: "2rem",
    paddingBlock: 0,
    paddingInline: 0,
    width: "2rem",
  },
  text: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
  },
  control: {
    backgroundColor: "transparent",
    borderRadius: 0,
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
  },
  textarea: {
    backgroundColor: "transparent",
    borderRadius: 0,
    borderWidth: 0,
    boxShadow: {
      default: "none",
      ":focus-visible": "none",
    },
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    paddingBlock: "0.75rem",
    resize: "none",
  },
});

const inputGroupAddonAlignStyles = {
  "block-end": styles.addonBlockEnd,
  "block-start": styles.addonBlockStart,
  "inline-end": styles.addonInlineEnd,
  "inline-start": styles.addonInlineStart,
};

const inputGroupButtonSizeStyles = {
  "icon-sm": styles.buttonIconSm,
  "icon-xs": styles.buttonIconXs,
  sm: styles.buttonSm,
  xs: styles.buttonXs,
};

export {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
  InputGroupText,
  InputGroupTextarea,
};
