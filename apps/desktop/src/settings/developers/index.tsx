import { CliSettingsSections } from "./cli";
import { CloudApiSection } from "./cloud-api";
import { LocalApiSection } from "./local-api";

import { SettingsPageTitle } from "~/settings/page-title";

export { buildMcpConfiguration, getCliInstallNotification } from "./cli";

export function SettingsDevelopers() {
  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-8">
      <SettingsPageTitle title="Developers" />
      <CliSettingsSections />
      <CloudApiSection />
      <LocalApiSection />
    </div>
  );
}
