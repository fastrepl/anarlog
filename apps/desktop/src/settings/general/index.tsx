import { Trans } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { disable, enable } from "@tauri-apps/plugin-autostart";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import type { General, GeneralStorage } from "@hypr/store";

export { SettingsAccount } from "./account";
import { AppSettingsView } from "./app-settings";
import {
  CORE_TRANSCRIPTION_LANGUAGE_CODES,
  getAdditionalSpokenLanguages,
} from "./language";
import { MainLanguageView } from "./main-language";
import { NotificationSettingsView } from "./notification";
import { Permissions } from "./permissions";
import { SpokenLanguagesView } from "./spoken-languages";
import { StorageSettingsView } from "./storage";
import { ThemeSelector } from "./theme";
import { TimezoneSelector } from "./timezone";
import { WeekStartSelector } from "./week-start";

import { Data } from "~/settings/data";
import { SettingsPageTitle } from "~/settings/page-title";
import { useConfigValues } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";

function useSettingsForm() {
  const value = useConfigValues([
    "autostart",
    "auto_start_scheduled_meetings",
    "auto_stop_meetings",
    "floating_bar_enabled",
    "notification_detect",
    "telemetry_consent",
    "ai_language",
    "spoken_languages",
    "current_stt_provider",
  ] as const);

  const setPartialValues = settings.UI.useSetPartialValuesCallback(
    (row: Partial<General>) =>
      ({
        ...row,
        spoken_languages: row.spoken_languages
          ? JSON.stringify(row.spoken_languages)
          : undefined,
        ignored_platforms: row.ignored_platforms
          ? JSON.stringify(row.ignored_platforms)
          : undefined,
        included_platforms: row.included_platforms
          ? JSON.stringify(row.included_platforms)
          : undefined,
        ignored_recurring_series: row.ignored_recurring_series
          ? JSON.stringify(row.ignored_recurring_series)
          : undefined,
        ignored_events: row.ignored_events
          ? JSON.stringify(row.ignored_events)
          : undefined,
      }) satisfies Partial<GeneralStorage>,
    [],
    settings.STORE_ID,
  );

  const form = useForm({
    defaultValues: {
      autostart: value.autostart,
      auto_start_scheduled_meetings: value.auto_start_scheduled_meetings,
      auto_stop_meetings: value.auto_stop_meetings,
      floating_bar_enabled: value.floating_bar_enabled,
      notification_detect: value.notification_detect,
      telemetry_consent: value.telemetry_consent,
      ai_language: value.ai_language,
      spoken_languages: getAdditionalSpokenLanguages(
        value.ai_language,
        value.spoken_languages,
      ),
    },
    listeners: {
      onChange: ({ formApi }) => {
        const {
          form: { errors },
        } = formApi.getAllErrors();
        if (errors.length > 0) {
          console.log(errors);
        }
        void formApi.handleSubmit();
      },
    },
    onSubmit: ({ value }) => {
      const normalizedValue = {
        ...value,
        spoken_languages: getAdditionalSpokenLanguages(
          value.ai_language,
          value.spoken_languages,
        ),
      };

      setPartialValues(normalizedValue);

      if (normalizedValue.autostart) {
        void enable();
      } else {
        void disable();
      }

      void analyticsCommands.event({
        event: "settings_changed",
        autostart: normalizedValue.autostart,
        auto_start_scheduled_meetings:
          normalizedValue.auto_start_scheduled_meetings,
        auto_stop_meetings: normalizedValue.auto_stop_meetings,
        floating_bar_enabled: normalizedValue.floating_bar_enabled,
        notification_detect: normalizedValue.notification_detect,
        telemetry_consent: normalizedValue.telemetry_consent,
      });
      void analyticsCommands.setProperties({
        set: {
          telemetry_opt_out: normalizedValue.telemetry_consent === false,
        },
      });
    },
  });

  return { form, value };
}

export function SettingsApp() {
  const { form } = useSettingsForm();

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>App</Trans>} />
      <div className="flex flex-col gap-4">
        <ThemeSelector />
        <form.Field name="autostart">
          {(autostartField) => (
            <form.Field name="auto_start_scheduled_meetings">
              {(autoStartScheduledMeetingsField) => (
                <form.Field name="auto_stop_meetings">
                  {(autoStopMeetingsField) => (
                    <form.Field name="floating_bar_enabled">
                      {(floatingBarEnabledField) => (
                        <form.Field name="telemetry_consent">
                          {(telemetryConsentField) => (
                            <AppSettingsView
                              autostart={{
                                value: autostartField.state.value,
                                onChange: (val) =>
                                  autostartField.handleChange(val),
                              }}
                              autoStartScheduledMeetings={{
                                value:
                                  autoStartScheduledMeetingsField.state.value,
                                onChange: (val) =>
                                  autoStartScheduledMeetingsField.handleChange(
                                    val,
                                  ),
                              }}
                              autoStopMeetings={{
                                value: autoStopMeetingsField.state.value,
                                onChange: (val) =>
                                  autoStopMeetingsField.handleChange(val),
                              }}
                              floatingBar={{
                                value: floatingBarEnabledField.state.value,
                                onChange: (val) =>
                                  floatingBarEnabledField.handleChange(val),
                              }}
                              telemetryConsent={{
                                value: telemetryConsentField.state.value,
                                onChange: (val) =>
                                  telemetryConsentField.handleChange(val),
                              }}
                            />
                          )}
                        </form.Field>
                      )}
                    </form.Field>
                  )}
                </form.Field>
              )}
            </form.Field>
          )}
        </form.Field>
      </div>

      <div>
        <h2 className="mb-4 font-sans text-lg font-semibold">
          <Trans>Language &amp; Region</Trans>
        </h2>
        <div className="flex flex-col gap-6">
          <form.Field name="ai_language">
            {(field) => (
              <MainLanguageView
                value={field.state.value}
                onChange={(val) => {
                  field.handleChange(val);
                  form.setFieldValue(
                    "spoken_languages",
                    getAdditionalSpokenLanguages(
                      val,
                      form.state.values.spoken_languages,
                    ),
                  );
                }}
                supportedLanguages={CORE_TRANSCRIPTION_LANGUAGE_CODES}
              />
            )}
          </form.Field>
          <TimezoneSelector />
          <WeekStartSelector />
          <form.Field name="spoken_languages">
            {(field) => (
              <SpokenLanguagesView
                mainLanguage={form.state.values.ai_language}
                value={field.state.value}
                onChange={(val) =>
                  field.handleChange(
                    getAdditionalSpokenLanguages(
                      form.state.values.ai_language,
                      val,
                    ),
                  )
                }
                supportedLanguages={CORE_TRANSCRIPTION_LANGUAGE_CODES}
              />
            )}
          </form.Field>
        </div>
      </div>
    </div>
  );
}

export function SettingsData() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Data</Trans>} />
      <StorageSettingsView />
      <Data />
    </div>
  );
}

export function SettingsNotifications() {
  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title={<Trans>Notifications</Trans>} />
      <NotificationSettingsView />
    </div>
  );
}

export function SettingsPermissions() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Permissions</Trans>} />
      <Permissions />
    </div>
  );
}
