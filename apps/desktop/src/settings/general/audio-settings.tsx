import { Trans, useLingui } from "@lingui/react/macro";

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

export function AudioSettingsView({
  audioRetention,
  microphoneDevice,
  rememberSpeakers,
}: {
  audioRetention: {
    value: string;
    onChange: (value: string) => void;
  };
  microphoneDevice: {
    value: string;
    devices: string[];
    onChange: (value: string) => void;
  };
  rememberSpeakers: {
    value: boolean;
    onChange: (value: boolean) => void;
  };
}) {
  return (
    <div className="flex flex-col gap-4">
      <MicrophoneRow
        value={microphoneDevice.value}
        devices={microphoneDevice.devices}
        onChange={microphoneDevice.onChange}
      />
      <AudioRetentionRow
        value={audioRetention.value}
        onChange={audioRetention.onChange}
      />
      <SettingSwitchRow
        title={<Trans>Remember speakers</Trans>}
        description={
          <Trans>
            Build voiceprints from meeting audio so speakers you name in a
            transcript are recognized in later meetings. Voiceprints never leave
            this device, and unnamed ones are deleted after 45 days.
          </Trans>
        }
        checked={rememberSpeakers.value}
        onChange={rememberSpeakers.onChange}
      />
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
