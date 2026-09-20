import { Trans } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";

import { SettingSwitchRow } from "~/settings/setting-row";

interface SettingItem {
  value: boolean;
  onChange: (value: boolean) => void;
}

export function MeetingSettingsView({
  autoJoinScheduledMeetings,
  autoStartScheduledMeetings,
  autoStopMeetings,
  autoRecordDetectedMeetings,
  floatingBar,
  meetingDisclosureAutoPost,
  captureMeetingChat,
  liveAssistEnabled,
}: {
  autoJoinScheduledMeetings: SettingItem;
  autoStartScheduledMeetings: SettingItem;
  autoStopMeetings: SettingItem;
  autoRecordDetectedMeetings: SettingItem;
  floatingBar: SettingItem;
  meetingDisclosureAutoPost: SettingItem;
  captureMeetingChat: SettingItem;
  liveAssistEnabled: SettingItem;
}) {
  const currentPlatform = platform();
  const supportsMeetingAx =
    currentPlatform === "macos" || currentPlatform === "linux";
  const supportsMicDetection = currentPlatform !== "windows";

  return (
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
          <Trans>Open the meeting link when a scheduled meeting begins.</Trans>
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
      {supportsMeetingAx && (
        <>
          <SettingSwitchRow
            title={<Trans>Start when a meeting app is detected</Trans>}
            description={
              <Trans>
                Use Accessibility to detect Zoom, Google Meet, or Teams open
                with an active call, and start listening automatically, even
                without a scheduled meeting.
              </Trans>
            }
            checked={autoRecordDetectedMeetings.value}
            onChange={autoRecordDetectedMeetings.onChange}
          />
          <SettingSwitchRow
            title={<Trans>Post recording disclosure in meeting chat</Trans>}
            description={
              <Trans>
                Tell participants when listening starts; this does not confirm
                consent.
              </Trans>
            }
            checked={meetingDisclosureAutoPost.value}
            onChange={meetingDisclosureAutoPost.onChange}
          />
          <SettingSwitchRow
            title={<Trans>Capture meeting chat in Memos</Trans>}
            description={
              <Trans>
                Save visible chat from supported meetings using Accessibility.
              </Trans>
            }
            checked={captureMeetingChat.value}
            onChange={captureMeetingChat.onChange}
          />
        </>
      )}
      <SettingSwitchRow
        title={<Trans>Show floating bar</Trans>}
        description={
          <Trans>Control listening without reopening BlackMushi.</Trans>
        }
        checked={floatingBar.value}
        onChange={floatingBar.onChange}
      />
      <SettingSwitchRow
        title={<Trans>Live Assist</Trans>}
        description={
          <Trans>
            Show AI suggestions (catch-up, action items, follow-up questions) in
            a side panel while a meeting is being transcribed live. Sends
            transcript excerpts to your configured AI provider every minute.
          </Trans>
        }
        checked={liveAssistEnabled.value}
        onChange={liveAssistEnabled.onChange}
      />
    </div>
  );
}
