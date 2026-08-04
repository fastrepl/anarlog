import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

import {
  SETTING_CONTROL_CLASS,
  SettingRow,
  SettingSwitchRow,
} from "~/settings/setting-row";

interface SettingItem {
  value: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}

interface AppSettingsViewProps {
  autostart: SettingItem;
  automaticUpdates: SettingItem;
  autoJoinScheduledMeetings: SettingItem;
  autoStartScheduledMeetings: SettingItem;
  autoStopMeetings: SettingItem;
  floatingBar: SettingItem;
  showAppInDock: SettingItem;
  showTrayIcon: SettingItem;
  telemetryConsent: SettingItem;
  meetingDisclosureAutoPost: SettingItem;
  captureMeetingChat: SettingItem;
  audioRetention: {
    value: string;
    onChange: (value: string) => void;
  };
  microphoneDevice: {
    value: string;
    devices: string[];
    onChange: (value: string) => void;
  };
}

export function AppSettingsView({
  autostart,
  automaticUpdates,
  autoJoinScheduledMeetings,
  autoStartScheduledMeetings,
  autoStopMeetings,
  floatingBar,
  showAppInDock,
  showTrayIcon,
  telemetryConsent,
  meetingDisclosureAutoPost,
  captureMeetingChat,
  audioRetention,
  microphoneDevice,
}: AppSettingsViewProps) {
  const currentPlatform = platform();
  const isMacos = currentPlatform === "macos";
  const supportsMicDetection = currentPlatform !== "windows";

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
          <Trans>Meetings</Trans>
        </h2>
        <div className="flex flex-col gap-4">
          <SettingSwitchRow
            title={<Trans>Start when meeting begins</Trans>}
            description={
              <Trans>Start listening when a scheduled meeting begins.</Trans>
            }
            checked={autoStartScheduledMeetings.value}
            onChange={autoStartScheduledMeetings.onChange}
          />
          <SettingSwitchRow
            title={<Trans>Join scheduled meetings</Trans>}
            description={
              <Trans>Open the meeting link when listening starts.</Trans>
            }
            checked={autoJoinScheduledMeetings.value}
            onChange={autoJoinScheduledMeetings.onChange}
            disabled={!autoStartScheduledMeetings.value}
          />
          {supportsMicDetection && (
            <SettingSwitchRow
              title={<Trans>Stop when meeting ends</Trans>}
              description={<Trans>Stop listening when your call ends.</Trans>}
              checked={autoStopMeetings.value}
              onChange={autoStopMeetings.onChange}
            />
          )}
          {isMacos && (
            <>
              <SettingSwitchRow
                title={<Trans>Post recording disclosure in meeting chat</Trans>}
                description={
                  <Trans>
                    Tell participants when listening starts; this does not
                    confirm consent.
                  </Trans>
                }
                checked={meetingDisclosureAutoPost.value}
                onChange={meetingDisclosureAutoPost.onChange}
              />
              <SettingSwitchRow
                title={<Trans>Capture meeting chat in Memos</Trans>}
                description={
                  <Trans>
                    Save visible chat from supported meetings using
                    Accessibility.
                  </Trans>
                }
                checked={captureMeetingChat.value}
                onChange={captureMeetingChat.onChange}
              />
            </>
          )}
          {isMacos && (
            <SettingSwitchRow
              title={<Trans>Show floating bar</Trans>}
              description={
                <Trans>Control listening without reopening Anarlog.</Trans>
              }
              checked={floatingBar.value}
              onChange={floatingBar.onChange}
            />
          )}
          <AudioRetentionRow
            value={audioRetention.value}
            onChange={audioRetention.onChange}
          />
          <MicrophoneRow
            value={microphoneDevice.value}
            devices={microphoneDevice.devices}
            onChange={microphoneDevice.onChange}
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

const SYSTEM_DEFAULT_MICROPHONE = "__system_default_microphone__";

function MicrophoneRow({
  value,
  devices,
  onChange,
}: {
  value: string;
  devices: string[];
  onChange: (value: string) => void;
}) {
  const availableDevices = [...new Set(devices)].sort((a, b) =>
    a.localeCompare(b),
  );
  const selectedDeviceUnavailable =
    Boolean(value) && !availableDevices.includes(value);
  if (selectedDeviceUnavailable) {
    availableDevices.unshift(value);
  }

  return (
    <SettingRow
      title={<Trans>Microphone</Trans>}
      description={
        <Trans>Choose the microphone that captures your voice.</Trans>
      }
    >
      {(labelProps) => (
        <Select
          value={value || SYSTEM_DEFAULT_MICROPHONE}
          onValueChange={(device) =>
            onChange(device === SYSTEM_DEFAULT_MICROPHONE ? "" : device)
          }
        >
          <SelectTrigger {...labelProps} className={SETTING_CONTROL_CLASS}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent className="max-h-64">
            <SelectItem value={SYSTEM_DEFAULT_MICROPHONE}>
              <Trans>Current default</Trans>
            </SelectItem>
            {availableDevices.map((device) => (
              <SelectItem key={device} value={device}>
                {device}
                {selectedDeviceUnavailable && device === value ? (
                  <>
                    {" "}
                    <Trans>(Unavailable — using current default)</Trans>
                  </>
                ) : null}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}
    </SettingRow>
  );
}

const AUDIO_RETENTION_OPTIONS = [
  "none",
  "oneDay",
  "threeDays",
  "oneWeek",
  "oneMonth",
  "forever",
] as const;

function AudioRetentionRow({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const { t } = useLingui();
  const copyByValue = {
    none: t`Don't save`,
    oneDay: t`1 day`,
    threeDays: t`3 days`,
    oneWeek: t`1 week`,
    oneMonth: t`1 month`,
    forever: t`Forever`,
  } as const;

  return (
    <SettingRow
      title={<Trans>Audio file retention</Trans>}
      description={
        <Trans>Choose how long recordings stay on this device.</Trans>
      }
    >
      {(labelProps) => (
        <Select value={value} onValueChange={onChange}>
          <SelectTrigger {...labelProps} className={SETTING_CONTROL_CLASS}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {AUDIO_RETENTION_OPTIONS.map((option) => (
              <SelectItem key={option} value={option}>
                {copyByValue[option]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}
    </SettingRow>
  );
}
