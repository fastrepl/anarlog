import { ConfigureProviders } from "./configure";
import { SttSettingsProvider } from "./context";
import { SelectProviderAndModel } from "./select";

import { SettingsPageTitle } from "~/settings/page-title";

export function STT() {
  return (
    <SttSettingsProvider>
      <div className="flex flex-col gap-6">
        <SettingsPageTitle title="Transcription" />
        <SelectProviderAndModel />
        <ConfigureProviders />
      </div>
    </SttSettingsProvider>
  );
}
