import { useLingui } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import { useEffect } from "react";

import { DEVICE_AUTH_REASON } from "~/lock/auth";
import { useAppLock } from "~/lock/store";
import { privacyMessages } from "~/settings/general/app-settings";
import { SettingsPageTitle } from "~/settings/page-title";
import {
  useSetSettingValues,
  useStoredSettingValuesQuery,
} from "~/settings/queries";
import { SettingSwitchRow } from "~/settings/setting-row";
import { resolveConfigValue } from "~/shared/config";

export function SettingsPrivacy() {
  const { i18n, t } = useLingui();
  const settingsQuery = useStoredSettingValuesQuery();
  const setSettingValues = useSetSettingValues();
  const available = useAppLock((state) => state.available);
  const authenticating = useAppLock((state) => state.authenticating);
  const authenticate = useAppLock((state) => state.authenticate);
  const lockApp = useAppLock((state) => state.lockApp);
  const refreshAvailability = useAppLock((state) => state.refreshAvailability);

  useEffect(() => {
    void refreshAvailability();
  }, [refreshAvailability]);

  if (settingsQuery.error) {
    throw settingsQuery.error;
  }
  if (settingsQuery.isLoading || !settingsQuery.data) {
    return null;
  }

  const posthogEnabled = resolveConfigValue(
    "telemetry_consent",
    settingsQuery.data,
  );
  const sentryEnabled = resolveConfigValue(
    "crash_reporting_consent",
    settingsQuery.data,
  );
  const lockAppEnabled = resolveConfigValue("lock_app", settingsQuery.data);
  const authAvailable = available === true;
  const lockAppDescription = !authAvailable
    ? t`Device authentication is not available on this computer.`
    : platform() === "windows"
      ? t`Require Windows Hello face, PIN, or password when opening Anarlog.`
      : t`Require Touch ID or your password when opening Anarlog.`;

  return (
    <div {...stylex.props(styles.page)}>
      <SettingsPageTitle title={i18n._(privacyMessages.title)} />

      <section {...stylex.props(styles.settings)}>
        <SettingSwitchRow
          title={t`Lock app`}
          description={lockAppDescription}
          checked={lockAppEnabled && authAvailable}
          disabled={!authAvailable || authenticating}
          onChange={(next) => {
            void (async () => {
              const canAuth = await refreshAvailability();
              if (!canAuth) return;
              const ok = await authenticate(
                DEVICE_AUTH_REASON.changeLockSettings,
              );
              if (!ok) return;
              setSettingValues({ lock_app: next });
              if (next) lockApp();
            })();
          }}
        />
        <SettingSwitchRow
          title={`${i18n._(privacyMessages.posthogTitle)} (PostHog)`}
          description={i18n._(privacyMessages.posthogDescription)}
          checked={posthogEnabled}
          onChange={(telemetryConsent) => {
            setSettingValues({ telemetry_consent: telemetryConsent });
          }}
        />
        <SettingSwitchRow
          title={t`Sentry`}
          description={t`Send sanitized crash and error reports to help improve Anarlog.`}
          checked={sentryEnabled}
          onChange={(crashReportingConsent) => {
            setSettingValues({
              crash_reporting_consent: crashReportingConsent,
            });
          }}
        />
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
