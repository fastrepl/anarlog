import { t } from "@lingui/core/macro";
import { ArrowSquareOut } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";

import { CliSettingsSections } from "./cli";
import { CloudApiSection } from "./cloud-api";
import { DevtoolsSection } from "./devtools";
import { WebhooksSection } from "./webhooks";

import { SettingsPageTitle } from "~/settings/page-title";

export { buildMcpConfiguration, getCliInstallNotification } from "./cli";

const DEVELOPERS_GUIDE_URL = "https://docs.anarlog.so/agents/overview";

export function SettingsDevelopers() {
  return (
    <div {...stylex.props(styles.page)}>
      <div {...stylex.props(styles.header)}>
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
          <ArrowSquareOut {...stylex.props(styles.icon)} />
        </Button>
      </div>
      <CliSettingsSections />
      <CloudApiSection />
      <WebhooksSection />
      <DevtoolsSection />
    </div>
  );
}

const styles = stylex.create({
  header: {
    alignItems: "center",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "2rem",
  },
});
