import { useForm } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";
import { disable, enable } from "@tauri-apps/plugin-autostart";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as listenerCommands } from "@hypr/plugin-listener";
import type { General, GeneralStorage } from "@hypr/store";

import { AccountSettings } from "./account";
import { AppSettingsView } from "./app-settings";
import { Audio } from "./audio";
import { MainLanguageView } from "./main-language";
import { NotificationSettingsView } from "./notification";
import { Permissions } from "./permissions";
import { SpokenLanguagesView } from "./spoken-languages";
import { StorageSettingsView } from "./storage";
import { TimezoneSelector } from "./timezone";
import { WeekStartSelector } from "./week-start";

import { Data } from "~/settings/data";
import { useConfigValues } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";

function useSettingsForm() {
  const value = useConfigValues([
    "autostart",
    "notification_detect",
    "save_recordings",
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
      notification_detect: value.notification_detect,
      save_recordings: value.save_recordings,
      telemetry_consent: value.telemetry_consent,
      ai_language: value.ai_language,
      spoken_languages: value.spoken_languages,
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
      setPartialValues(value);

      if (value.autostart) {
        void enable();
      } else {
        void disable();
      }

      void analyticsCommands.event({
        event: "settings_changed",
        autostart: value.autostart,
        notification_detect: value.notification_detect,
        save_recordings: value.save_recordings,
        telemetry_consent: value.telemetry_consent,
      });
      void analyticsCommands.setProperties({
        set: {
          telemetry_opt_out: value.telemetry_consent === false,
        },
      });
    },
  });

  return { form, value };
}

export function SettingsAccount() {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="w-full max-w-md">
        <AccountSettings />
      </div>
    </div>
  );
}

export function SettingsApp() {
  const { form } = useSettingsForm();

  const supportedLanguagesQuery = useQuery({
    queryKey: ["documented-language-codes", "live"],
    queryFn: async () => {
      const result = await listenerCommands.listDocumentedLanguageCodesLive();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    staleTime: Infinity,
  });
  const supportedLanguages = supportedLanguagesQuery.data ?? ["en"];

  return (
    <div className="flex flex-col gap-8 pt-3">
      <form.Field name="autostart">
        {(autostartField) => (
          <form.Field name="notification_detect">
            {(notificationDetectField) => (
              <form.Field name="save_recordings">
                {(saveRecordingsField) => (
                  <form.Field name="telemetry_consent">
                    {(telemetryConsentField) => (
                      <AppSettingsView
                        autostart={{
                          title: "Start Char at login",
                          description:
                            "Always ready without manually launching.",
                          value: autostartField.state.value,
                          onChange: (val) => autostartField.handleChange(val),
                        }}
                        notificationDetect={{
                          title: "Auto-detect meetings",
                          description:
                            "Automatically start and stop listening when a meeting is detected.",
                          value: notificationDetectField.state.value,
                          onChange: (val) =>
                            notificationDetectField.handleChange(val),
                        }}
                        saveRecordings={{
                          title: "Save recordings",
                          description:
                            "Keep audio files locally on your device.",
                          value: saveRecordingsField.state.value,
                          onChange: (val) =>
                            saveRecordingsField.handleChange(val),
                        }}
                        telemetryConsent={{
                          title: "Share usage data",
                          description:
                            "Send anonymous usage analytics to help improve Char.",
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

      <div>
        <h2 className="mb-4 font-serif text-lg font-semibold">
          Language & Region
        </h2>
        <div className="flex flex-col gap-6">
          <form.Field name="ai_language">
            {(field) => (
              <MainLanguageView
                value={field.state.value}
                onChange={(val) => field.handleChange(val)}
                supportedLanguages={supportedLanguages}
              />
            )}
          </form.Field>
          <TimezoneSelector />
          <WeekStartSelector />
        </div>
      </div>

      <div>
        <h2 className="mb-4 font-serif text-lg font-semibold">Transcription</h2>
        <div className="flex flex-col gap-6">
          <form.Field name="spoken_languages">
            {(field) => (
              <SpokenLanguagesView
                value={field.state.value}
                onChange={(val) => field.handleChange(val)}
                supportedLanguages={supportedLanguages}
              />
            )}
          </form.Field>
        </div>
      </div>

      <StorageSettingsView />

      <div>
        <h2 className="mb-4 font-serif text-lg font-semibold">Data</h2>
        <Data />
      </div>
    </div>
  );
}

export function SettingsNotifications() {
  return (
    <div className="pt-3">
      <NotificationSettingsView />
    </div>
  );
}

export function SettingsSystem() {
  return (
    <div className="flex flex-col gap-8 pt-3">
      <Permissions />
      <Audio />
    </div>
  );
}
