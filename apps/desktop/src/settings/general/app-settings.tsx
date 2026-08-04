import { Trans } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";

import { SettingSwitchRow } from "~/settings/setting-row";

interface SettingItem {
  value: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}

interface AppSettingsViewProps {
  autostart: SettingItem;
  automaticUpdates: SettingItem;
  showAppInDock: SettingItem;
  showTrayIcon: SettingItem;
  telemetryConsent: SettingItem;
}

export function AppSettingsView({
  autostart,
  automaticUpdates,
  showAppInDock,
  showTrayIcon,
  telemetryConsent,
}: AppSettingsViewProps) {
  const currentPlatform = platform();
  const isMacos = currentPlatform === "macos";

  return (
    <div className="flex flex-col gap-8">
      <section>
        <div className="flex flex-col gap-4">
          <SettingSwitchRow
            title={<Trans>Start Anarlog at login</Trans>}
            description={<Trans>Have Anarlog ready when you sign in.</Trans>}
            checked={autostart.value}
            onChange={autostart.onChange}
          />
          <SettingSwitchRow
            title={<Trans>Automatically install updates</Trans>}
            description={
              <Trans>
                Stay current with updates installed the next time Anarlog opens.
              </Trans>
            }
            checked={automaticUpdates.value}
            onChange={automaticUpdates.onChange}
          />
          {isMacos && (
            <SettingSwitchRow
              title={<Trans>Show app in Dock</Trans>}
              description={
                <Trans>Show Anarlog in the Dock and app switcher.</Trans>
              }
              checked={showAppInDock.value}
              onChange={showAppInDock.onChange}
            />
          )}
          <SettingSwitchRow
            title={<Trans>Show tray icon</Trans>}
            description={
              isMacos ? (
                <Trans>Open Anarlog from the menu bar.</Trans>
              ) : undefined
            }
            checked={showTrayIcon.value}
            onChange={showTrayIcon.onChange}
          />
        </div>
      </section>

      <section>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Privacy</Trans>
        </h2>
        <div className="flex flex-col gap-4">
          <SettingSwitchRow
            title={<Trans>Share usage data</Trans>}
            description={
              <Trans>Help improve Anarlog with anonymous usage data.</Trans>
            }
            checked={telemetryConsent.value}
            onChange={telemetryConsent.onChange}
          />
        </div>
      </section>
    </div>
  );
}
