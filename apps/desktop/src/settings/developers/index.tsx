import { CliSettingsSections } from "./cli";
import { CloudApiSection } from "./cloud-api";
import { DevtoolsSection } from "./devtools";
import { WebhooksSection } from "./webhooks";

import { SettingsPageTitle } from "~/settings/page-title";

export { buildMcpConfiguration, getCliInstallNotification } from "./cli";

export function SettingsDevelopers() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title="Developers" />
      <CliSettingsSections />
      <CloudApiSection />
      <WebhooksSection />
      <DevtoolsSection />
    </div>
  );
}
