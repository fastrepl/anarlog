import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";

import { ConfigureProviders } from "./configure";
import { LlmSettingsProvider } from "./context";
import { SelectProviderAndModel } from "./select";

import { SettingsPageTitle } from "~/settings/page-title";

export function LLM() {
  return (
    <LlmSettingsProvider>
      <div {...stylex.props(styles.page)}>
        <SettingsPageTitle title={<Trans>Intelligence</Trans>} />
        <SelectProviderAndModel />
        <ConfigureProviders />
      </div>
    </LlmSettingsProvider>
  );
}

const styles = stylex.create({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "1.5rem",
  },
});
