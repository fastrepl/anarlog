import { Trans } from "@lingui/react/macro";
import { ArrowSquareOut } from "@phosphor-icons/react";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";

import { MeetingImportScreen } from "~/imports/screen";
import { SettingsPageTitle } from "~/settings/page-title";

const IMPORTS_DOCUMENTATION_URL = "https://docs.anarlog.so/imports";

export function SettingsImports() {
  return (
    <div className="flex flex-col gap-8">
      <div className="flex items-center justify-between gap-4">
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
          <ArrowSquareOut className="size-3.5" />
        </Button>
      </div>
      <MeetingImportScreen />
    </div>
  );
}
