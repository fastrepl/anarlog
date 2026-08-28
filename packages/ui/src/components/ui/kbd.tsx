import * as stylex from "@stylexjs/stylex";

import { colors, fonts, spacing } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

function Kbd({
  className,
  style,
  sx,
  ...props
}: React.ComponentProps<"kbd"> & StyleXProps) {
  return (
    <kbd
      data-slot="kbd"
      {...props}
      {...mergeStyleXProps([styles.root, sx], className, style)}
    />
  );
}

function KbdGroup({
  className,
  style,
  sx,
  ...props
}: React.ComponentProps<"div"> & StyleXProps) {
  return (
    <div
      data-slot="kbd-group"
      {...props}
      {...mergeStyleXProps([styles.group, sx], className, style)}
    />
  );
}

const styles = stylex.create({
  root: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: "0.25rem",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow:
      "0 1px 0 0 var(--kbd-shadow-outer), inset 0 1px 0 0 var(--kbd-shadow-inset)",
    color: colors.mutedForeground,
    display: "inline-flex",
    flexShrink: 0,
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: spacing.xs,
    justifyContent: "center",
    lineHeight: 1,
    minWidth: "1.25rem",
    paddingInline: spacing.xs,
    pointerEvents: "none",
    userSelect: "none",
    whiteSpace: "nowrap",
    height: {
      default: "1.25rem",
      ":is(*) svg": "0.75rem",
    },
    width: {
      default: "fit-content",
      ":is(*) svg": "0.75rem",
    },
  },
  group: {
    alignItems: "center",
    display: "inline-flex",
    gap: "0.125rem",
  },
});

export { Kbd, KbdGroup };
