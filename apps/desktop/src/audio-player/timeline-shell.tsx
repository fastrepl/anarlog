import * as stylex from "@stylexjs/stylex";
import type { MouseEvent, ReactNode } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

export function TimelineMeta({ children }: { children: ReactNode }) {
  return <div {...stylex.props(styles.meta)}>{children}</div>;
}

export function TimelineShell({
  contentClassName,
  leading,
  meta,
  main,
  onContextMenu,
}: {
  contentClassName?: string;
  leading: ReactNode;
  meta?: ReactNode;
  main: ReactNode;
  onContextMenu?: (event: MouseEvent<HTMLDivElement>) => void;
}) {
  return (
    <div {...stylex.props(styles.root)} onContextMenu={onContextMenu}>
      <div {...mergeStyleXProps(styles.content, contentClassName)}>
        {leading}
        {meta}
        <div {...stylex.props(styles.main)}>{main}</div>
      </div>
    </div>
  );
}

const styles = stylex.create({
  content: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    maxWidth: "100%",
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
    width: "100%",
  },
  main: {
    flex: "1",
    minWidth: 0,
  },
  meta: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "inline-flex",
    flexShrink: 0,
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
    fontVariantNumeric: "tabular-nums",
    gap: "0.25rem",
    userSelect: "none",
  },
  root: {
    backgroundColor: "transparent",
    borderRadius: radii.xl,
    userSelect: "none",
    width: "100%",
  },
});
