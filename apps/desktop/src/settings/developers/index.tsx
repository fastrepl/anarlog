import { t } from "@lingui/core/macro";
import { ArrowSquareOut } from "@phosphor-icons/react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";

import { CliSettingsSections } from "./cli";
import { CloudApiSection } from "./cloud-api";
import { WebhooksSection } from "./webhooks";

import { SettingsPageTitle } from "~/settings/page-title";

export { buildMcpConfiguration, getCliInstallNotification } from "./cli";

const DEVELOPERS_GUIDE_URL = "https://docs.anarlog.so/agents/overview";

export function SettingsDevelopers() {
  return (
    <div className="flex flex-col gap-8">
      <div className="flex items-center justify-between gap-4">
        <SettingsPageTitle title={t`Developers`} />
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() =>
            void openerCommands.openUrl(DEVELOPERS_GUIDE_URL, null)
          }
        >
          {t`Guide`}
          <ArrowSquareOut className="size-3.5" />
        </Button>
      </div>
      <CliSettingsSections />
      <CloudApiSection />
      <WebhooksSection />
    </div>
  );
}
