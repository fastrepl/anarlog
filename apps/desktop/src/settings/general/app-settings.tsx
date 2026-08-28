import { msg } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";

import { SettingSwitchRow } from "~/settings/setting-row";

export const privacyMessages = {
  title: msg`Privacy`,
  posthogTitle: msg`Share usage data`,
  posthogDescription: msg`Help improve Anarlog with anonymous usage data.`,
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
  const currentPlatform = platform();
  const isMacos = currentPlatform === "macos";

  return (
    <div {...stylex.props(styles.page)}>
      <section>
        <div {...stylex.props(styles.settings)}>
          {!appStoreBuild && (
            <>
              <SettingSwitchRow
                title={<Trans>Start Anarlog at login</Trans>}
                description={
                  <Trans>Have Anarlog ready when you sign in.</Trans>
                }
                checked={autostart.value}
                onChange={autostart.onChange}
              />
              <SettingSwitchRow
                title={<Trans>Automatically install updates</Trans>}
                description={
                  <Trans>
                    Stay current with updates installed the next time Anarlog
                    opens.
                  </Trans>
                }
                checked={automaticUpdates.value}
                onChange={automaticUpdates.onChange}
              />
            </>
          )}
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
    </div>
  );
}

const styles = stylex.create({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "2rem",
  },
  settings: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
});
