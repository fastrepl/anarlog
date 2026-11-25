import { msg } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { platform } from "@tauri-apps/plugin-os";

import { Button } from "@anlg/ui/components/ui/button";

import { SettingSwitchRow } from "~/settings/setting-row";
import { useTabs } from "~/store/zustand/tabs";

export const privacyMessages = {
  title: msg`Privacy`,
  posthogTitle: msg`Share usage data`,
};

interface SettingItem {
  value: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}

interface AppSettingsViewProps {
  appStoreBuild: boolean;
  autostart: SettingItem;
  automaticUpdates: SettingItem;
  showAppInDock: SettingItem;
  showTrayIcon: SettingItem;
}

export function AppSettingsView({
  appStoreBuild,
  autostart,
  automaticUpdates,
  showAppInDock,
  showTrayIcon,
}: AppSettingsViewProps) {
  const openNew = useTabs((state) => state.openNew);
  const version = useQuery({
    queryKey: ["app-version"],
    queryFn: getVersion,
    staleTime: Infinity,
  });
  const currentPlatform = platform();
  const isMacos = currentPlatform === "macos";

  return (
    <div className="flex flex-col gap-8">
      <section>
        <div className="flex flex-col gap-4">
          {!appStoreBuild && (
            <>
              <SettingSwitchRow
                title={<Trans>Start Anarlog at login</Trans>}
                checked={autostart.value}
                onChange={autostart.onChange}
              />
              <SettingSwitchRow
                title={<Trans>Automatically install updates</Trans>}
                checked={automaticUpdates.value}
                onChange={automaticUpdates.onChange}
              />
            </>
          )}
          <Button
            variant="outline"
            className="self-start"
            disabled={!version.data}
            onClick={() => {
              if (version.data) {
                openNew({
                  type: "changelog",
                  state: { current: version.data, previous: version.data },
                });
              }
            }}
          >
            <Trans>Show release notes</Trans>
          </Button>
          {isMacos && (
            <SettingSwitchRow
              title={<Trans>Show app in Dock</Trans>}
              checked={showAppInDock.value}
              onChange={showAppInDock.onChange}
            />
          )}
          <SettingSwitchRow
            title={<Trans>Show tray icon</Trans>}
            checked={showTrayIcon.value}
            onChange={showTrayIcon.onChange}
          />
        </div>
      </section>
    </div>
  );
}
