import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { CircleNotch } from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands as listenerCommands } from "@anlg/plugin-transcription";

export { SettingsAccount } from "./account";
import { AppSettingsView } from "./app-settings";
import { AudioSettingsView } from "./audio-settings";
import {
  CORE_TRANSCRIPTION_LANGUAGE_CODES,
  getAdditionalSpokenLanguages,
} from "./language";
import { MainLanguageView } from "./main-language";
import { MeetingSettingsView } from "./meeting-settings";
import { NotificationSettingsView } from "./notification";
import { Permissions } from "./permissions";
import { SpokenLanguagesView } from "./spoken-languages";
import { StorageSettingsView } from "./storage";
import { SummaryLengthSelector } from "./summary-length";
import { TimezoneSelector } from "./timezone";
import { WeekStartSelector } from "./week-start";

import { SettingsPageTitle } from "~/settings/page-title";
import {
  type StoredSettingValues,
  useSetSettingValues,
  useStoredSettingValuesQuery,
} from "~/settings/queries";
import { isAppStoreBuild } from "~/shared/app-store";
import { resolveConfigValue, resolveConfigValues } from "~/shared/config";

const SETTINGS_FORM_KEYS = [
  "autostart",
  "automatic_updates",
  "auto_join_scheduled_meetings",
  "auto_start_scheduled_meetings",
  "auto_stop_meetings",
  "floating_bar_enabled",
  "show_app_in_dock",
  "show_tray_icon",
  "notification_detect",
  "consent_auto_send_chat",
  "capture_meeting_chat",
  "ai_language",
  "spoken_languages",
  "current_stt_provider",
] as const;

function useSettingsForm(storedSettings: StoredSettingValues) {
  const settingsValue = resolveConfigValues(SETTINGS_FORM_KEYS, storedSettings);

  const setSettingValues = useSetSettingValues();

  const form = useForm({
    defaultValues: {
      autostart: settingsValue.autostart,
      automatic_updates: settingsValue.automatic_updates,
      auto_join_scheduled_meetings: settingsValue.auto_join_scheduled_meetings,
      auto_start_scheduled_meetings:
        settingsValue.auto_start_scheduled_meetings,
      auto_stop_meetings: settingsValue.auto_stop_meetings,
      floating_bar_enabled: settingsValue.floating_bar_enabled,
      show_app_in_dock: settingsValue.show_app_in_dock,
      show_tray_icon: settingsValue.show_tray_icon,
      notification_detect: settingsValue.notification_detect,
      consent_auto_send_chat: settingsValue.consent_auto_send_chat,
      capture_meeting_chat: settingsValue.capture_meeting_chat,
      ai_language: settingsValue.ai_language,
      spoken_languages: getAdditionalSpokenLanguages(
        settingsValue.ai_language,
        settingsValue.spoken_languages,
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

      setSettingValues({
        autostart: normalizedValue.autostart,
        automatic_updates: normalizedValue.automatic_updates,
        auto_join_scheduled_meetings:
          normalizedValue.auto_join_scheduled_meetings,
        auto_start_scheduled_meetings:
          normalizedValue.auto_start_scheduled_meetings,
        auto_stop_meetings: normalizedValue.auto_stop_meetings,
        floating_bar_enabled: normalizedValue.floating_bar_enabled,
        show_app_in_dock: normalizedValue.show_app_in_dock,
        show_tray_icon: normalizedValue.show_tray_icon,
        notification_detect: normalizedValue.notification_detect,
        consent_auto_send_chat: normalizedValue.consent_auto_send_chat,
        capture_meeting_chat: normalizedValue.capture_meeting_chat,
        ai_language: normalizedValue.ai_language,
        spoken_languages: JSON.stringify(normalizedValue.spoken_languages),
      });

      void analyticsCommands.event({
        event: "settings_changed",
        autostart: normalizedValue.autostart,
        automatic_updates: normalizedValue.automatic_updates,
        auto_join_scheduled_meetings:
          normalizedValue.auto_join_scheduled_meetings,
        auto_start_scheduled_meetings:
          normalizedValue.auto_start_scheduled_meetings,
        auto_stop_meetings: normalizedValue.auto_stop_meetings,
        floating_bar_enabled: normalizedValue.floating_bar_enabled,
        show_app_in_dock: normalizedValue.show_app_in_dock,
        show_tray_icon: normalizedValue.show_tray_icon,
        notification_detect: normalizedValue.notification_detect,
        consent_auto_send_chat: normalizedValue.consent_auto_send_chat,
        capture_meeting_chat: normalizedValue.capture_meeting_chat,
      });
    },
  });

  // form.setFieldValue never reaches the form-level onChange listener when
  // the field has no mounted Field instance, so submit explicitly.
  const submitFieldValue: (typeof form)["setFieldValue"] = (field, value) => {
    form.setFieldValue(field, value);
    void form.handleSubmit();
  };

  return { form, submitFieldValue, value: settingsValue };
}

type SettingsSection = "app" | "meetings";

export function SettingsApp() {
  return <SettingsSectionPage section="app" />;
}

export function SettingsMeetings() {
  return <SettingsSectionPage section="meetings" />;
}

function SettingsSectionPage({ section }: { section: SettingsSection }) {
  const { data, isLoading, error } = useStoredSettingValuesQuery();

  if (error) {
    throw error;
  }
  if (isLoading || !data) {
    return (
      <div className="flex min-h-48 items-center justify-center">
        <CircleNotch
          aria-label={t`Loading settings`}
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  return <SettingsSectionContent section={section} storedSettings={data} />;
}

function SettingsSectionContent({
  section,
  storedSettings,
}: {
  section: SettingsSection;
  storedSettings: StoredSettingValues;
}) {
  const { form, submitFieldValue } = useSettingsForm(storedSettings);
  const setSettingValues = useSetSettingValues();
  const audioRetention =
    resolveConfigValue("audio_retention", storedSettings) || "forever";
  const rememberSpeakers =
    resolveConfigValue("remember_speakers", storedSettings) === true;
  const microphoneDevice = resolveConfigValue(
    "microphone_device",
    storedSettings,
  );
  const speakerDevice = resolveConfigValue("speaker_device", storedSettings);
  const microphoneDevicesQuery = useQuery({
    queryKey: ["microphone-devices"],
    queryFn: async () => {
      const result = await listenerCommands.listMicrophoneDevices();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    enabled: section === "meetings",
    refetchInterval: 3_000,
  });
  const speakerDevicesQuery = useQuery({
    queryKey: ["speaker-devices"],
    queryFn: async () => {
      const result = await listenerCommands.listSpeakerDevices();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    enabled: section === "meetings",
    refetchInterval: 3_000,
  });

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle
        title={
          section === "app" ? <Trans>General</Trans> : <Trans>Meetings</Trans>
        }
      />

      {section === "app" && (
        <>
          <form.Subscribe selector={(state) => state.values}>
            {(values) => (
              <AppSettingsView
                appStoreBuild={isAppStoreBuild()}
                autostart={{
                  value: values.autostart,
                  onChange: (value) => submitFieldValue("autostart", value),
                }}
                automaticUpdates={{
                  value: values.automatic_updates,
                  onChange: (value) =>
                    submitFieldValue("automatic_updates", value),
                }}
                showAppInDock={{
                  value: values.show_app_in_dock,
                  onChange: (value) =>
                    submitFieldValue("show_app_in_dock", value),
                }}
                showTrayIcon={{
                  value: values.show_tray_icon,
                  onChange: (value) =>
                    submitFieldValue("show_tray_icon", value),
                }}
              />
            )}
          </form.Subscribe>

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

          <StorageSettingsView />
        </>
      )}

      {section === "meetings" && (
        <>
          <form.Subscribe selector={(state) => state.values}>
            {(values) => (
              <MeetingSettingsView
                autoJoinScheduledMeetings={{
                  value: values.auto_join_scheduled_meetings,
                  onChange: (value) =>
                    submitFieldValue("auto_join_scheduled_meetings", value),
                }}
                autoStartScheduledMeetings={{
                  value: values.auto_start_scheduled_meetings,
                  onChange: (value) =>
                    submitFieldValue("auto_start_scheduled_meetings", value),
                }}
                autoStopMeetings={{
                  value: values.auto_stop_meetings,
                  onChange: (value) =>
                    submitFieldValue("auto_stop_meetings", value),
                }}
                floatingBar={{
                  value: values.floating_bar_enabled,
                  onChange: (value) =>
                    submitFieldValue("floating_bar_enabled", value),
                }}
                meetingDisclosureAutoPost={{
                  value: values.consent_auto_send_chat,
                  onChange: (value) =>
                    submitFieldValue("consent_auto_send_chat", value),
                }}
                captureMeetingChat={{
                  value: values.capture_meeting_chat,
                  onChange: (value) =>
                    submitFieldValue("capture_meeting_chat", value),
                }}
              />
            )}
          </form.Subscribe>

          <div>
            <h2 className="mb-4 font-sans text-lg font-semibold">
              <Trans>Summaries</Trans>
            </h2>
            <SummaryLengthSelector />
          </div>

          <div>
            <h2 className="mb-4 font-sans text-lg font-semibold">
              <Trans>Audio</Trans>
            </h2>
            <AudioSettingsView
              audioRetention={{
                value: audioRetention,
                onChange: (value) =>
                  setSettingValues({
                    audio_retention: value,
                    save_recordings: value !== "none",
                  }),
              }}
              microphoneDevice={{
                value: microphoneDevice,
                devices: microphoneDevicesQuery.data ?? [],
                onChange: (value) =>
                  setSettingValues({ microphone_device: value }),
              }}
              speakerDevice={{
                value: speakerDevice,
                devices: speakerDevicesQuery.data ?? [],
                onChange: (value) =>
                  setSettingValues({ speaker_device: value }),
              }}
              rememberSpeakers={{
                value: rememberSpeakers,
                onChange: (value) =>
                  setSettingValues({ remember_speakers: value }),
              }}
            />
          </div>
        </>
      )}
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
