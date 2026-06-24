import { Trans } from "@lingui/react/macro";

import { ConfigureProviders } from "./configure";
import { LlmSettingsProvider } from "./context";
import { SelectProviderAndModel } from "./select";

import { SettingsPageTitle } from "~/settings/page-title";

export function LLM() {
  return (
    <LlmSettingsProvider>
      <div className="flex flex-col gap-6">
        <SettingsPageTitle title={<Trans>Intelligence</Trans>} />
        <SelectProviderAndModel />
        <ConfigureProviders />
      </div>
    </LlmSettingsProvider>
  );
}
