import { Trans } from "@lingui/react/macro";

import { MeetingImportScreen } from "~/imports/screen";
import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsImports() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Imports</Trans>} />
      <MeetingImportScreen />
    </div>
  );
}
