import { Trans } from "@lingui/react/macro";

import { ConfigureProviders } from "./configure";
import { SttSettingsProvider } from "./context";
import {
  SelectProviderAndModel,
  TranscriptionLanguageWarningBanner,
} from "./select";

import { SettingsPageTitle } from "~/settings/page-title";

export function STT() {
  return (
    <SttSettingsProvider>
      <TranscriptionLanguageWarningBanner />
      <div className="flex flex-col gap-6">
        <SettingsPageTitle title={<Trans>Transcription</Trans>} />
        <SelectProviderAndModel />
        <ConfigureProviders />
      </div>
    </SttSettingsProvider>
  );
}
