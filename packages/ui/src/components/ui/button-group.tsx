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
    buttonGroupSelectorClassName,
    buttonGroupOrientationSelectorClassNames[resolvedOrientation],
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
        cn([
          buttonGroupSelectorClassName,
          buttonGroupOrientationSelectorClassNames[resolvedOrientation],
          className,
        ]),
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
  const resolvedStyle = mergeStyleXProps(
    [styles.text, sx],
    cn([buttonGroupTextSelectorClassName, className]),
    style,
  );

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

const buttonGroupSelectorClassName =
  "focus-visible:*:relative focus-visible:*:z-10 has-[>[data-slot=button-group]]:gap-2 [&>[data-slot=select-trigger]:last-of-type]:has-[select[aria-hidden=true]:last-child]:rounded-r-full [&>[data-slot=select-trigger]:not([class*='w-'])]:w-fit [&>input]:flex-1";

const buttonGroupOrientationSelectorClassNames = {
  horizontal:
    "[&>*:not(:first-child)]:rounded-l-none [&>*:not(:first-child)]:border-l-0 [&>*:not(:last-child)]:rounded-r-none",
  vertical:
    "[&>*:not(:first-child)]:rounded-t-none [&>*:not(:first-child)]:border-t-0 [&>*:not(:last-child)]:rounded-b-none",
};

const buttonGroupTextSelectorClassName =
  "[&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4";

const styles = stylex.create({
  group: {
    alignItems: "stretch",
    display: "flex",
    width: "fit-content",
  },
  groupHorizontal: {
    flexDirection: "row",
  },
  groupVertical: {
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
    lineHeight: "1.25rem",
    paddingInline: "1rem",
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
