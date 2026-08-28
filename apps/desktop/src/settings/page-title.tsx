import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import { fonts } from "@anlg/design-system/tokens.stylex";

export function SettingsPageTitle({ title }: { title: ReactNode }) {
  return <h2 {...stylex.props(styles.title)}>{title}</h2>;
}

const styles = stylex.create({
  title: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    fontWeight: 600,
    letterSpacing: "0em",
    lineHeight: 1,
  },
});
