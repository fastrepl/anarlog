import { Trans } from "@lingui/react/macro";

import { Switch } from "@hypr/ui/components/ui/switch";

interface SettingItem {
  title: string;
  description: string;
  value: boolean;
  onChange: (value: boolean) => void;
}

interface AppSettingsViewProps {
  autostart: SettingItem;
  autoStartScheduledMeetings: SettingItem;
  autoStopMeetings: SettingItem;
  telemetryConsent: SettingItem;
}

export function AppSettingsView({
  autostart,
  autoStartScheduledMeetings,
  autoStopMeetings,
  telemetryConsent,
}: AppSettingsViewProps) {
  return (
    <div>
      <h2 className="mb-4 font-serif text-lg font-semibold">
        <Trans>App</Trans>
      </h2>
      <div className="flex flex-col gap-4">
        <SettingRow
          title={autostart.title}
          description={autostart.description}
          checked={autostart.value}
          onChange={autostart.onChange}
        />
        <SettingRow
          title={autoStartScheduledMeetings.title}
          description={autoStartScheduledMeetings.description}
          checked={autoStartScheduledMeetings.value}
          onChange={autoStartScheduledMeetings.onChange}
        />
        <SettingRow
          title={autoStopMeetings.title}
          description={autoStopMeetings.description}
          checked={autoStopMeetings.value}
          onChange={autoStopMeetings.onChange}
        />
        <SettingRow
          title={telemetryConsent.title}
          description={telemetryConsent.description}
          checked={telemetryConsent.value}
          onChange={telemetryConsent.onChange}
        />
      </div>
    </div>
  );
}

function SettingRow({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="flex-1">
        <h3 className="mb-1 text-sm font-medium">{title}</h3>
        <p className="text-xs text-neutral-600">{description}</p>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} />
    </div>
  );
}
