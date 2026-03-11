import { Switch } from "@hypr/ui/components/ui/switch";

interface SettingItem {
  title: string;
  description: string;
  value: boolean;
  onChange: (value: boolean) => void;
}

interface AppSettingsViewProps {
  autostart: SettingItem;
  notificationDetect: SettingItem;
  saveRecordings: SettingItem;
  telemetryConsent: SettingItem;
}

export function AppSettingsView({
  autostart,
  notificationDetect,
  saveRecordings,
  telemetryConsent,
}: AppSettingsViewProps) {
  return (
    <div>
      <h2 className="mb-4 font-serif text-lg font-semibold">App</h2>
      <div className="flex flex-col gap-4">
        <SettingRow
          title={autostart.title}
          description={autostart.description}
          checked={autostart.value}
          onChange={autostart.onChange}
        />
        <SettingRow
          title={notificationDetect.title}
          description={notificationDetect.description}
          checked={notificationDetect.value}
          onChange={notificationDetect.onChange}
        />
        <SettingRow
          title={saveRecordings.title}
          description={saveRecordings.description}
          checked={saveRecordings.value}
          onChange={saveRecordings.onChange}
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
