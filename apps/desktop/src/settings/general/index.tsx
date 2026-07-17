import { Trans } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Loader2Icon } from "lucide-react";
import { useState } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { getE2eeIdentityStatus } from "@hypr/plugin-db";

export { SettingsAccount } from "./account";
import { AppSettingsView } from "./app-settings";
import { E2eeSetupDialog } from "./e2ee-setup";
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

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { applyCloudsyncPreference } from "~/auth/cloudsync";
import { SettingsPageTitle } from "~/settings/page-title";
import {
  setSettingValue,
  type StoredSettingValues,
  useSetSettingValues,
  useStoredSettingValuesQuery,
} from "~/settings/queries";
import { resolveConfigValue, resolveConfigValues } from "~/shared/config";

const SETTINGS_FORM_KEYS = [
  "autostart",
  "auto_join_scheduled_meetings",
  "auto_start_scheduled_meetings",
  "auto_stop_meetings",
  "floating_bar_enabled",
  "show_app_in_dock",
  "show_tray_icon",
  "notification_detect",
  "telemetry_consent",
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
      auto_join_scheduled_meetings: settingsValue.auto_join_scheduled_meetings,
      auto_start_scheduled_meetings:
        settingsValue.auto_start_scheduled_meetings,
      auto_stop_meetings: settingsValue.auto_stop_meetings,
      floating_bar_enabled: settingsValue.floating_bar_enabled,
      show_app_in_dock: settingsValue.show_app_in_dock,
      show_tray_icon: settingsValue.show_tray_icon,
      notification_detect: settingsValue.notification_detect,
      telemetry_consent: settingsValue.telemetry_consent,
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
        auto_join_scheduled_meetings:
          normalizedValue.auto_join_scheduled_meetings,
        auto_start_scheduled_meetings:
          normalizedValue.auto_start_scheduled_meetings,
        auto_stop_meetings: normalizedValue.auto_stop_meetings,
        floating_bar_enabled: normalizedValue.floating_bar_enabled,
        show_app_in_dock: normalizedValue.show_app_in_dock,
        show_tray_icon: normalizedValue.show_tray_icon,
        notification_detect: normalizedValue.notification_detect,
        telemetry_consent: normalizedValue.telemetry_consent,
        consent_auto_send_chat: normalizedValue.consent_auto_send_chat,
        capture_meeting_chat: normalizedValue.capture_meeting_chat,
        ai_language: normalizedValue.ai_language,
        spoken_languages: JSON.stringify(normalizedValue.spoken_languages),
      });

      void analyticsCommands.event({
        event: "settings_changed",
        autostart: normalizedValue.autostart,
        auto_join_scheduled_meetings:
          normalizedValue.auto_join_scheduled_meetings,
        auto_start_scheduled_meetings:
          normalizedValue.auto_start_scheduled_meetings,
        auto_stop_meetings: normalizedValue.auto_stop_meetings,
        floating_bar_enabled: normalizedValue.floating_bar_enabled,
        show_app_in_dock: normalizedValue.show_app_in_dock,
        show_tray_icon: normalizedValue.show_tray_icon,
        notification_detect: normalizedValue.notification_detect,
        telemetry_consent: normalizedValue.telemetry_consent,
        consent_auto_send_chat: normalizedValue.consent_auto_send_chat,
        capture_meeting_chat: normalizedValue.capture_meeting_chat,
      });
      void analyticsCommands.setProperties({
        set: {
          telemetry_opt_out: normalizedValue.telemetry_consent === false,
        },
      });
    },
  });

  return { form, value: settingsValue };
}

export function SettingsApp() {
  const { data, isLoading, error } = useStoredSettingValuesQuery();

  if (error) {
    throw error;
  }
  if (isLoading || !data) {
    return (
      <div className="flex min-h-48 items-center justify-center">
        <Loader2Icon
          aria-label="Loading settings"
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  return <SettingsAppContent storedSettings={data} />;
}

function SettingsAppContent({
  storedSettings,
}: {
  storedSettings: StoredSettingValues;
}) {
  const { form } = useSettingsForm(storedSettings);
  const auth = useAuth();
  const { isPro } = useBillingAccess();
  const [e2eeSetupOpen, setE2eeSetupOpen] = useState(false);
  const storedCloudSyncEnabled = resolveConfigValue(
    "cloud_sync_enabled",
    storedSettings,
  );
  const e2eeIdentityQuery = useQuery({
    queryKey: ["e2ee-identity", auth.session?.user.id],
    queryFn: () => getE2eeIdentityStatus(auth.session!.user.id),
    enabled: Boolean(auth.session?.user.id),
  });
  const cloudSyncMutation = useMutation({
    mutationKey: ["cloudsync-preference"],
    mutationFn: async (enabled: boolean) => {
      await setSettingValue("cloud_sync_enabled", enabled);
      const result = await applyCloudsyncPreference(auth.session);
      if (result === "account_mismatch") {
        await auth.signOut();
      }
    },
    onError: (error) => {
      console.error("[cloudsync] failed to apply sync preference", error);
    },
  });
  const e2eePreflightMutation = useMutation({
    mutationKey: ["e2ee-preflight"],
    mutationFn: async () => {
      const accountUserId = auth.session?.user.id;
      if (!accountUserId) {
        throw new Error("Sign in before enabling encrypted cloud sync");
      }
      return getE2eeIdentityStatus(accountUserId);
    },
    onSuccess: ({ configured }) => {
      if (configured) {
        cloudSyncMutation.mutate(true);
      } else {
        setE2eeSetupOpen(true);
      }
    },
  });
  const cloudSyncEnabled = cloudSyncMutation.isPending
    ? (cloudSyncMutation.variables ?? storedCloudSyncEnabled)
    : storedCloudSyncEnabled && e2eeIdentityQuery.data?.configured !== false;

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>App</Trans>} />
      <div className="flex flex-col gap-4">
        <ThemeSelector />
        <form.Field name="autostart">
          {(autostartField) => (
            <form.Field name="auto_join_scheduled_meetings">
              {(autoJoinScheduledMeetingsField) => (
                <form.Field name="auto_start_scheduled_meetings">
                  {(autoStartScheduledMeetingsField) => (
                    <form.Field name="auto_stop_meetings">
                      {(autoStopMeetingsField) => (
                        <form.Field name="floating_bar_enabled">
                          {(floatingBarEnabledField) => (
                            <form.Field name="show_app_in_dock">
                              {(showAppInDockField) => (
                                <form.Field name="show_tray_icon">
                                  {(showTrayIconField) => (
                                    <form.Field name="telemetry_consent">
                                      {(telemetryConsentField) => (
                                        <form.Field name="consent_auto_send_chat">
                                          {(meetingDisclosureAutoPostField) => (
                                            <form.Field name="capture_meeting_chat">
                                              {(captureMeetingChatField) => (
                                                <AppSettingsView
                                                  autostart={{
                                                    value:
                                                      autostartField.state
                                                        .value,
                                                    onChange: (val) =>
                                                      autostartField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  autoJoinScheduledMeetings={{
                                                    value:
                                                      autoJoinScheduledMeetingsField
                                                        .state.value,
                                                    onChange: (val) =>
                                                      autoJoinScheduledMeetingsField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  autoStartScheduledMeetings={{
                                                    value:
                                                      autoStartScheduledMeetingsField
                                                        .state.value,
                                                    onChange: (val) =>
                                                      autoStartScheduledMeetingsField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  autoStopMeetings={{
                                                    value:
                                                      autoStopMeetingsField
                                                        .state.value,
                                                    onChange: (val) =>
                                                      autoStopMeetingsField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  floatingBar={{
                                                    value:
                                                      floatingBarEnabledField
                                                        .state.value,
                                                    onChange: (val) =>
                                                      floatingBarEnabledField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  showAppInDock={{
                                                    value:
                                                      showAppInDockField.state
                                                        .value,
                                                    onChange: (val) =>
                                                      showAppInDockField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  showTrayIcon={{
                                                    value:
                                                      showTrayIconField.state
                                                        .value,
                                                    onChange: (val) =>
                                                      showTrayIconField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  telemetryConsent={{
                                                    value:
                                                      telemetryConsentField
                                                        .state.value,
                                                    onChange: (val) =>
                                                      telemetryConsentField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  cloudSync={{
                                                    value: cloudSyncEnabled,
                                                    onChange: (enabled) => {
                                                      if (enabled) {
                                                        e2eePreflightMutation.mutate();
                                                      } else {
                                                        cloudSyncMutation.mutate(
                                                          false,
                                                        );
                                                      }
                                                    },
                                                    disabled:
                                                      !isPro ||
                                                      cloudSyncMutation.isPending ||
                                                      e2eePreflightMutation.isPending ||
                                                      e2eeIdentityQuery.isLoading,
                                                    available: isPro,
                                                  }}
                                                  meetingDisclosureAutoPost={{
                                                    value:
                                                      meetingDisclosureAutoPostField
                                                        .state.value,
                                                    onChange: (val) =>
                                                      meetingDisclosureAutoPostField.handleChange(
                                                        val,
                                                      ),
                                                  }}
                                                  captureMeetingChat={{
                                                    value:
                                                      captureMeetingChatField
                                                        .state.value,
                                                    onChange: (val) =>
                                                      captureMeetingChatField.handleChange(
                                                        val,
                                                      ),
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

      <StorageSettingsView />
      {auth.session?.user.id && (
        <E2eeSetupDialog
          open={e2eeSetupOpen}
          onOpenChange={setE2eeSetupOpen}
          accountUserId={auth.session.user.id}
          accessToken={auth.session.access_token}
          onReady={() => {
            setE2eeSetupOpen(false);
            void e2eeIdentityQuery.refetch();
            cloudSyncMutation.mutate(true);
          }}
        />
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
