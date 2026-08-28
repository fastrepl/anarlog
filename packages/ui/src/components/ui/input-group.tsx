import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

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
      {...mergeStyleXProps([styles.group, sx], className, style)}
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
        className,
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
      className={className}
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
      {...mergeStyleXProps([styles.text, sx], className, style)}
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

const styles = stylex.create({
  group: {
    alignItems: "center",
    backgroundColor: `light-dark(transparent, color-mix(in oklab, ${colors.input} 30%, transparent))`,
    borderColor: {
      default: colors.input,
      ":is(:has([data-slot][aria-invalid='true']))": colors.destructive,
    },
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: "0 1px rgb(0 0 0 / 0.05)",
      ":is(:has([data-slot='input-group-control']:focus-visible))": `0 0 0 1px ${colors.ring}`,
      ":is(:has([data-slot][aria-invalid='true']))": `0 0 0 1px light-dark(color-mix(in oklab, ${colors.destructive} 20%, transparent), color-mix(in oklab, ${colors.destructive} 40%, transparent))`,
    },
    display: "flex",
    flexDirection: {
      default: "row",
      ":is(:has(> [data-align='block-start']))": "column",
      ":is(:has(> [data-align='block-end']))": "column",
    },
    height: {
      default: "2.25rem",
      ":is(:has(> textarea))": "auto",
      ":is(:has(> [data-align='block-start']))": "auto",
      ":is(:has(> [data-align='block-end']))": "auto",
    },
    opacity: {
      default: 1,
      ":is([data-disabled='true']) > [data-slot='input-group-addon']": 0.5,
    },
    outlineColor: "transparent",
    outlineOffset: "2px",
    outlineStyle: "solid",
    outlineWidth: "2px",
    paddingBottom: {
      default: null,
      ":is(:has(> [data-align='block-start'])) > input": "0.75rem",
      ":is(:has(> input)) > [data-align='block-end']": "0.625rem",
    },
    paddingLeft: {
      default: null,
      ":is(:has(> [data-align='inline-start'])) > input": "0.5rem",
    },
    paddingRight: {
      default: null,
      ":is(:has(> [data-align='inline-end'])) > input": "0.5rem",
    },
    paddingTop: {
      default: null,
      ":is(:has(> [data-align='block-end'])) > input": "0.75rem",
      ":is(:has(> input)) > [data-align='block-start']": "0.625rem",
    },
    position: "relative",
    transitionDuration: "150ms",
    transitionProperty: "color, border-color, box-shadow",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  addon: {
    alignItems: "center",
    borderRadius: {
      default: null,
      ":is(*) > kbd": "calc(var(--radius, 0.5rem) - 5px)",
    },
    color: colors.mutedForeground,
    cursor: "text",
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: "0.5rem",
    height: {
      default: "auto",
      ":is(*) > svg": "1rem",
    },
    justifyContent: "center",
    lineHeight: "1.25rem",
    paddingBottom: "0.375rem",
    paddingTop: "0.375rem",
    userSelect: "none",
    width: {
      default: null,
      ":is(*) > svg": "1rem",
    },
  },
  addonInlineStart: {
    marginLeft: {
      default: 0,
      ":is(:has(> button))": "-0.45rem",
      ":is(:has(> kbd))": "-0.35rem",
    },
    order: -9999,
    paddingLeft: "0.75rem",
  },
  addonInlineEnd: {
    marginRight: {
      default: 0,
      ":is(:has(> button))": "-0.4rem",
      ":is(:has(> kbd))": "-0.35rem",
    },
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
    paddingInline: "0.5rem",
    height: {
      default: "1.5rem",
      ":is(*) > svg": "0.875rem",
    },
    width: {
      default: null,
      ":is(*) > svg": "0.875rem",
    },
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
    height: {
      default: null,
      ":is(*) svg": "1rem",
    },
    lineHeight: "1.25rem",
    pointerEvents: {
      default: null,
      ":is(*) svg": "none",
    },
    width: {
      default: null,
      ":is(*) svg": "1rem",
    },
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
