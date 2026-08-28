import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";

export function ResourceDetailEmpty({ message }: { message: ReactNode }) {
  return (
    <div {...stylex.props(styles.root)}>
      <p {...stylex.props(styles.message)}>{message}</p>
    </div>
  );
}

const styles = stylex.create({
  message: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
  },
  root: {
    alignItems: "center",
    display: "flex",
    height: "100%",
    justifyContent: "center",
  },
});
