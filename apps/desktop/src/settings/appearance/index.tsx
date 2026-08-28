import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";

import { AppIconSelector } from "./app-icon";
import { ThemeSelector } from "./theme";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsAppearance() {
  return (
    <div {...stylex.props(styles.page)}>
      <SettingsPageTitle title={<Trans>Appearance</Trans>} />
      <ThemeSelector />
      <AppIconSelector />
    </div>
  );
}

const styles = stylex.create({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "2.5rem",
    maxWidth: "64rem",
  },
});
