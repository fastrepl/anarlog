import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";

import { ParticipantInput } from "./input";

export function ParticipantsDisplay({ sessionId }: { sessionId: string }) {
  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.divider)} />
      <ParticipantInput sessionId={sessionId} />
    </div>
  );
}

const styles = stylex.create({
  divider: {
    backgroundColor: colors.accent,
    height: "1px",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
});
