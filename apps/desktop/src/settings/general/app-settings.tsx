import { Trans } from "@lingui/react/macro";
import { type ReactNode, useId } from "react";

import { Switch } from "@hypr/ui/components/ui/switch";

interface SettingItem {
  value: boolean;
  onChange: (value: boolean) => void;
}

interface AppSettingsViewProps {
  autostart: SettingItem;
  autoStartScheduledMeetings: SettingItem;
  autoStopMeetings: SettingItem;
  floatingBar: SettingItem;
  sidebarTimeline: SettingItem;
  telemetryConsent: SettingItem;
}

export function AppSettingsView({
  autostart,
  autoStartScheduledMeetings,
  autoStopMeetings,
  floatingBar,
  sidebarTimeline,
  telemetryConsent,
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
            title={<Trans>Show timeline in sidebar</Trans>}
            description={
              <Trans>
                Use the left sidebar timeline instead of the top timeline.
              </Trans>
            }
            checked={sidebarTimeline.value}
            onChange={sidebarTimeline.onChange}
          />
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
            title={<Trans>Show floating bar</Trans>}
            description={
              <Trans>Show the compact floating control while listening.</Trans>
            }
            checked={floatingBar.value}
            onChange={floatingBar.onChange}
          />
        </div>
      </section>
    </div>
  );
}

function SettingRow({
  title,
  description,
  checked,
  onChange,
}: {
  title: ReactNode;
  description: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
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
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      />
    </div>
  );
}
