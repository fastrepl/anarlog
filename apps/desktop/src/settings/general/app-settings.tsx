import { Trans, useLingui } from "@lingui/react/macro";
import { type ReactNode, useId } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { Switch } from "@hypr/ui/components/ui/switch";

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
  return (
    <div className="flex flex-col gap-8">
      <section>
        <div className="flex flex-col gap-4">
          <SettingRow
            title={<Trans>Start Anarlog at login</Trans>}
            description={
              <Trans>Always ready without manually launching.</Trans>
            }
            checked={autostart.value}
            onChange={autostart.onChange}
          />
          <SettingRow
            title={<Trans>Automatically install updates</Trans>}
            description={
              <Trans>
                Download updates in the background and install them the next
                time Anarlog opens.
              </Trans>
            }
            checked={automaticUpdates.value}
            onChange={automaticUpdates.onChange}
          />
          <SettingRow
            title={<Trans>Show app in Dock</Trans>}
            description={
              <Trans>Show Anarlog in the Dock and app switcher.</Trans>
            }
            checked={showAppInDock.value}
            onChange={showAppInDock.onChange}
          />
          <SettingRow
            title={<Trans>Show tray icon</Trans>}
            description={
              <Trans>Keep Anarlog available from the menu bar.</Trans>
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
          <SettingRow
            title={<Trans>Start when meeting begins</Trans>}
            description={
              <Trans>
                Automatically start listening when an event-backed note reaches
                its scheduled start time.
              </Trans>
            }
            checked={autoStartScheduledMeetings.value}
            onChange={autoStartScheduledMeetings.onChange}
          />
          <SettingRow
            title={<Trans>Join scheduled meetings</Trans>}
            description={
              <Trans>
                Automatically open the meeting link when scheduled listening
                starts.
              </Trans>
            }
            checked={autoJoinScheduledMeetings.value}
            onChange={autoJoinScheduledMeetings.onChange}
            disabled={!autoStartScheduledMeetings.value}
          />
          <SettingRow
            title={<Trans>Stop when meeting ends</Trans>}
            description={
              <Trans>
                Automatically stop listening when the meeting app releases the
                microphone.
              </Trans>
            }
            checked={autoStopMeetings.value}
            onChange={autoStopMeetings.onChange}
          />
          <SettingRow
            title={<Trans>Post recording disclosure in meeting chat</Trans>}
            description={
              <Trans>
                Automatically post a disclosure after listening starts when the
                active meeting chat supports safe posting. Posting failure does
                not stop listening. A disclosure does not confirm participant
                consent.
              </Trans>
            }
            checked={meetingDisclosureAutoPost.value}
            onChange={meetingDisclosureAutoPost.onChange}
          />
          <SettingRow
            title={<Trans>Capture meeting chat in Memos</Trans>}
            description={
              <Trans>
                While listening, use Accessibility access to copy visible chat
                from supported meeting apps and browser meetings into the active
                note.
              </Trans>
            }
            checked={captureMeetingChat.value}
            onChange={captureMeetingChat.onChange}
          />
          <SettingRow
            title={<Trans>Show floating bar</Trans>}
            description={
              <Trans>Show the compact floating control while listening.</Trans>
            }
            checked={floatingBar.value}
            onChange={floatingBar.onChange}
          />
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
          <SettingRow
            title={<Trans>Share usage data</Trans>}
            description={
              <Trans>
                Send anonymous usage analytics to help improve Anarlog.
              </Trans>
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
  const titleId = useId();
  const descriptionId = useId();
  const availableDevices = [...new Set(devices)].sort((a, b) =>
    a.localeCompare(b),
  );
  const selectedDeviceUnavailable =
    Boolean(value) && !availableDevices.includes(value);
  if (selectedDeviceUnavailable) {
    availableDevices.unshift(value);
  }

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 id={titleId} className="mb-1 text-sm font-medium">
          <Trans>Microphone</Trans>
        </h3>
        <p id={descriptionId} className="text-muted-foreground text-xs">
          <Trans>Use your microphone to capture your voice</Trans>
        </p>
      </div>
      <Select
        value={value || SYSTEM_DEFAULT_MICROPHONE}
        onValueChange={(device) =>
          onChange(device === SYSTEM_DEFAULT_MICROPHONE ? "" : device)
        }
      >
        <SelectTrigger
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
          className="bg-card h-9 w-48 shadow-none focus:ring-0"
        >
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
    </div>
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
  const titleId = useId();
  const descriptionId = useId();
  const copyByValue = {
    none: t`Don't save`,
    oneDay: t`1 day`,
    threeDays: t`3 days`,
    oneWeek: t`1 week`,
    oneMonth: t`1 month`,
    forever: t`Forever`,
  } as const;

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 id={titleId} className="mb-1 text-sm font-medium">
          <Trans>Audio file retention</Trans>
        </h3>
        <p id={descriptionId} className="text-muted-foreground text-xs">
          <Trans>How long recorded meeting audio is kept on this device.</Trans>
        </p>
      </div>
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
          className="bg-card h-9 w-48 shadow-none focus:ring-0"
        >
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
    </div>
  );
}

function SettingRow({
  title,
  description,
  checked,
  onChange,
  disabled = false,
}: {
  title: ReactNode;
  description: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  const titleId = useId();
  const descriptionId = useId();

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 id={titleId} className="mb-1 text-sm font-medium">
          {title}
        </h3>
        <p id={descriptionId} className="text-muted-foreground text-xs">
          {description}
        </p>
      </div>
      <Switch
        checked={checked}
        onCheckedChange={onChange}
        disabled={disabled}
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      />
    </div>
  );
}
