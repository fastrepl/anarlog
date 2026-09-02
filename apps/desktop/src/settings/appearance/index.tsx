import { Trans } from "@lingui/react/macro";

import { AppIconSelector } from "./app-icon";
import { SidebarItemFieldsSettings } from "./sidebar-item-fields";
import { ThemeSelector } from "./theme";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsAppearance() {
  return (
    <div className="flex max-w-5xl flex-col gap-10">
      <SettingsPageTitle title={<Trans>Appearance</Trans>} />
      <ThemeSelector />
      <AppIconSelector />
      <SidebarItemFieldsSettings />
    </div>
  );
}
