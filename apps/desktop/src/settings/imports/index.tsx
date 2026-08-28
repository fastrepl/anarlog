import { Trans } from "@lingui/react/macro";
import { ArrowSquareOut } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";

import { MeetingImportScreen } from "~/imports/screen";
import { SettingsPageTitle } from "~/settings/page-title";

const IMPORTS_DOCUMENTATION_URL = "https://docs.anarlog.so/imports";

export function SettingsImports() {
  return (
    <div {...stylex.props(styles.page)}>
      <div {...stylex.props(styles.header)}>
        <SettingsPageTitle title={<Trans>Imports</Trans>} />
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() =>
            void openerCommands.openUrl(IMPORTS_DOCUMENTATION_URL, null)
          }
        >
          <Trans>Documentation</Trans>
          <ArrowSquareOut {...stylex.props(styles.icon)} />
        </Button>
      </div>
      <MeetingImportScreen />
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
