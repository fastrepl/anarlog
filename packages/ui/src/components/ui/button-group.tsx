import { Slot } from "@radix-ui/react-slot";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Separator } from "@anlg/ui/components/ui/separator";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";
import { cn } from "@anlg/utils";

function buttonGroupVariants({
  orientation,
  className,
  class: classValue,
}: {
  class?: Parameters<typeof cn>[number];
  className?: Parameters<typeof cn>[number];
  orientation?: "horizontal" | "vertical" | null;
} = {}) {
  const resolvedOrientation = orientation ?? "horizontal";

  return cn([
    stylex.props([
      styles.group,
      buttonGroupOrientationStyles[resolvedOrientation],
    ]).className,
    classValue,
    className,
  ]);
}

function ButtonGroup({
  className,
  style,
  sx,
  orientation,
  ...props
}: React.ComponentProps<"div"> &
  StyleXProps & {
    orientation?: "horizontal" | "vertical" | null;
  }) {
  const resolvedOrientation = orientation ?? "horizontal";

  return (
    <div
      role="group"
      data-slot="button-group"
      data-orientation={orientation}
      {...props}
      {...mergeStyleXProps(
        [styles.group, buttonGroupOrientationStyles[resolvedOrientation], sx],
        className,
        style,
      )}
    />
  );
}

function ButtonGroupText({
  className,
  style,
  sx,
  asChild = false,
  ...props
}: React.ComponentProps<"div"> & {
  asChild?: boolean;
} & StyleXProps) {
  const Comp = asChild ? Slot : "div";
  const resolvedStyle = mergeStyleXProps([styles.text, sx], className, style);

  return (
    <Comp
      {...props}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
}

function ButtonGroupSeparator({
  className,
  style,
  sx,
  orientation = "vertical",
  ...props
}: React.ComponentProps<typeof Separator> & StyleXProps) {
  return (
    <Separator
      data-slot="button-group-separator"
      orientation={orientation}
      {...props}
      className={className}
      style={style}
      sx={[
        styles.separator,
        orientation === "vertical" && styles.separatorVertical,
        sx,
      ]}
    />
  );
}

const styles = stylex.create({
  group: {
    alignItems: "stretch",
    borderTopRightRadius: {
      default: null,
      ":is(*) > [data-slot='select-trigger']:last-of-type:has(select[aria-hidden='true']:last-child)":
        radii.full,
    },
    borderBottomRightRadius: {
      default: null,
      ":is(*) > [data-slot='select-trigger']:last-of-type:has(select[aria-hidden='true']:last-child)":
        radii.full,
    },
    display: "flex",
    flexBasis: {
      default: null,
      ":is(*) > input": "0%",
    },
    flexGrow: {
      default: null,
      ":is(*) > input": 1,
    },
    flexShrink: {
      default: null,
      ":is(*) > input": 1,
    },
    gap: {
      default: 0,
      ":is(:has(> [data-slot='button-group']))": "0.5rem",
    },
    position: {
      default: null,
      ":is(*) > *:focus-visible": "relative",
    },
    width: {
      default: "fit-content",
      ":is(*) > [data-slot='select-trigger']": "fit-content",
    },
    zIndex: {
      default: null,
      ":is(*) > *:focus-visible": 10,
    },
  },
  groupHorizontal: {
    borderBottomLeftRadius: {
      default: null,
      ":is(*) > *:not(:first-child)": 0,
    },
    borderBottomRightRadius: {
      default: null,
      ":is(*) > *:not(:last-child)": 0,
    },
    borderLeftWidth: {
      default: null,
      ":is(*) > *:not(:first-child)": 0,
    },
    borderTopLeftRadius: {
      default: null,
      ":is(*) > *:not(:first-child)": 0,
    },
    borderTopRightRadius: {
      default: null,
      ":is(*) > *:not(:last-child)": 0,
    },
    flexDirection: "row",
  },
  groupVertical: {
    borderBottomLeftRadius: {
      default: null,
      ":is(*) > *:not(:last-child)": 0,
    },
    borderBottomRightRadius: {
      default: null,
      ":is(*) > *:not(:last-child)": 0,
    },
    borderTopLeftRadius: {
      default: null,
      ":is(*) > *:not(:first-child)": 0,
    },
    borderTopRightRadius: {
      default: null,
      ":is(*) > *:not(:first-child)": 0,
    },
    borderTopWidth: {
      default: null,
      ":is(*) > *:not(:first-child)": 0,
    },
    flexDirection: "column",
  },
  text: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 1px rgb(0 0 0 / 0.05)",
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: "0.5rem",
    height: {
      default: null,
      ":is(*) svg": "1rem",
    },
    lineHeight: "1.25rem",
    paddingInline: "1rem",
    pointerEvents: {
      default: null,
      ":is(*) svg": "none",
    },
    width: {
      default: null,
      ":is(*) svg": "1rem",
    },
  },
  separator: {
    alignSelf: "stretch",
    backgroundColor: colors.input,
    margin: 0,
    position: "relative",
  },
  separatorVertical: {
    height: "auto",
  },
});

const buttonGroupOrientationStyles = {
  horizontal: styles.groupHorizontal,
  vertical: styles.groupVertical,
};

export {
  ButtonGroup,
  ButtonGroupSeparator,
  ButtonGroupText,
  buttonGroupVariants,
};
