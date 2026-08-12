import { useLingui } from "@lingui/react/macro";

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

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={i18n._(privacyMessages.title)} />

      <section className="flex flex-col gap-4">
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
