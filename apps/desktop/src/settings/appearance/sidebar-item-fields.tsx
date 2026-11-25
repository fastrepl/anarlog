import { Trans } from "@lingui/react/macro";

import { useSetSettingValue } from "~/settings/queries";
import { SettingSwitchRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";

export function SidebarItemFieldsSettings() {
  const showFolder = useConfigValue("sidebar_show_folder");
  const showTags = useConfigValue("sidebar_show_tags");
  const setShowFolder = useSetSettingValue("sidebar_show_folder");
  const setShowTags = useSetSettingValue("sidebar_show_tags");

  return (
    <section className="flex flex-col gap-4">
      <h3 className="text-lg font-semibold">
        <Trans>Notes list</Trans>
      </h3>
      <div className="flex flex-col gap-4">
        <SettingSwitchRow
          title={<Trans>Folder</Trans>}
          checked={showFolder}
          onChange={setShowFolder}
        />
        <SettingSwitchRow
          title={<Trans>Tags</Trans>}
          checked={showTags}
          onChange={setShowTags}
        />
      </div>
    </section>
  );
}
