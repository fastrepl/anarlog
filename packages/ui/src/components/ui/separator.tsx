import * as SeparatorPrimitive from "@radix-ui/react-separator";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const Separator = React.forwardRef<
  React.ComponentRef<typeof SeparatorPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof SeparatorPrimitive.Root> & StyleXProps
>(
  (
    {
      className,
      style,
      sx,
      orientation = "horizontal",
      decorative = true,
      ...props
    },
    ref,
  ) => (
    <SeparatorPrimitive.Root
      {...props}
      {...mergeStyleXProps(
        [
          styles.root,
          orientation === "horizontal" ? styles.horizontal : styles.vertical,
          sx,
        ],
        className,
        style,
      )}
      ref={ref}
      decorative={decorative}
      orientation={orientation}
    />
  ),
);
Separator.displayName = SeparatorPrimitive.Root.displayName;

const styles = stylex.create({
  root: {
    backgroundColor: colors.border,
    flexShrink: 0,
  },
  horizontal: {
    height: "1px",
    width: "100%",
  },
  vertical: {
    height: "100%",
    width: "1px",
  },
});

export { Separator };
