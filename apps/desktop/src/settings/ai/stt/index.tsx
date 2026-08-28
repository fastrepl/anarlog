import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";

import { ConfigureProviders } from "./configure";
import { SttSettingsProvider } from "./context";
import { SelectProviderAndModel } from "./select";

import { SettingsPageTitle } from "~/settings/page-title";

export function STT() {
  return (
    <SttSettingsProvider>
      <div {...stylex.props(styles.page)}>
        <SettingsPageTitle title={<Trans>Transcription</Trans>} />
        <SelectProviderAndModel />
        <ConfigureProviders />
      </div>
    </SttSettingsProvider>
  );
}

const styles = stylex.create({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "1.5rem",
  },
});
