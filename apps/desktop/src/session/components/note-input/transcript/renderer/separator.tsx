import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";

export function TranscriptSeparator() {
  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.rule)} />
      <span>~ ~ ~ ~ ~ ~ ~ ~ ~</span>
      <div {...stylex.props(styles.rule)} />
    </div>
  );
}

const styles = stylex.create({
  root: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 300,
    gap: "0.75rem",
  },
  rule: {
    borderColor: `color-mix(in srgb, ${colors.border} 40%, transparent)`,
    borderStyle: "solid",
    borderWidth: 0,
    borderTopWidth: "1px",
    flex: "1",
  },
});
