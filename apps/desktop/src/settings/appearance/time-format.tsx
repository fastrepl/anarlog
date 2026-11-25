import { Trans } from "@lingui/react/macro";

import { useSetSettingValue } from "~/settings/queries";
import { SettingSwitchRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";

export function TimeFormatSettings() {
  const enabled = useConfigValue("use_24_hour_time");
  const setEnabled = useSetSettingValue("use_24_hour_time");

  return (
    <SettingSwitchRow
      title={<Trans>Use 24-hour time</Trans>}
      checked={enabled}
      onChange={setEnabled}
    />
  );
}
