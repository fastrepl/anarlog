import { Trans } from "@lingui/react/macro";
import { type ReactNode, useId } from "react";

import { Switch } from "@hypr/ui/components/ui/switch";

interface SettingItem {
  value: boolean;
  onChange: (value: boolean) => void;
}

interface AppSettingsViewProps {
  autostart: SettingItem;
  autoJoinScheduledMeetings: SettingItem;
  autoStartScheduledMeetings: SettingItem;
  autoStopMeetings: SettingItem;
  floatingBar: SettingItem;
  showAppInDock: SettingItem;
  showTrayIcon: SettingItem;
  telemetryConsent: SettingItem;
  captureMeetingChat: SettingItem;
}

export function AppSettingsView({
  autostart,
  autoJoinScheduledMeetings,
  autoStartScheduledMeetings,
  autoStopMeetings,
  floatingBar,
  showAppInDock,
  showTrayIcon,
  telemetryConsent,
  captureMeetingChat,
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
            title={<Trans>Share usage data</Trans>}
            description={
              <Trans>
                Send anonymous usage analytics to help improve Anarlog.
              </Trans>
            }
            checked={telemetryConsent.value}
            onChange={telemetryConsent.onChange}
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
