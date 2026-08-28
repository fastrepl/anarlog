import { Trans, useLingui } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

import {
  settingControlStyles,
  SettingRow,
  SettingSwitchRow,
} from "~/settings/setting-row";

export function AudioSettingsView({
  audioRetention,
  microphoneDevice,
  speakerDevice,
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
  speakerDevice: {
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
    <div {...stylex.props(styles.settings)}>
      <AudioDeviceRow
        title={<Trans>Microphone</Trans>}
        description={
          <Trans>Choose the microphone that captures your voice.</Trans>
        }
        value={microphoneDevice.value}
        devices={microphoneDevice.devices}
        onChange={microphoneDevice.onChange}
      />
      <AudioDeviceRow
        title={<Trans>Speakers</Trans>}
        description={
          <Trans>
            Choose the speakers that play other participants so Anarlog records
            them.
          </Trans>
        }
        value={speakerDevice.value}
        devices={speakerDevice.devices}
        onChange={speakerDevice.onChange}
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

const SYSTEM_DEFAULT_DEVICE = "__system_default_device__";

function AudioDeviceRow({
  title,
  description,
  value,
  devices,
  onChange,
}: {
  title: ReactNode;
  description: ReactNode;
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
    <SettingRow title={title} description={description}>
      {(labelProps) => (
        <Select
          value={value || SYSTEM_DEFAULT_DEVICE}
          onValueChange={(device) =>
            onChange(device === SYSTEM_DEFAULT_DEVICE ? "" : device)
          }
        >
          <SelectTrigger {...labelProps} sx={settingControlStyles.control}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent sx={styles.menu}>
            <SelectItem value={SYSTEM_DEFAULT_DEVICE}>
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
          <SelectTrigger {...labelProps} sx={settingControlStyles.control}>
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

const styles = stylex.create({
  menu: {
    maxHeight: "16rem",
  },
  settings: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
});
