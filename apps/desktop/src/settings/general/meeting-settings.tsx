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
  floatingBar,
  meetingDisclosureAutoPost,
  captureMeetingChat,
}: {
  autoJoinScheduledMeetings: SettingItem;
  autoStartScheduledMeetings: SettingItem;
  autoStopMeetings: SettingItem;
  floatingBar: SettingItem;
  meetingDisclosureAutoPost: SettingItem;
  captureMeetingChat: SettingItem;
}) {
  const currentPlatform = platform();
  const isMacos = currentPlatform === "macos";
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
          <SettingSwitchRow
            title={<Trans>Show floating bar</Trans>}
            description={
              <Trans>Control listening without reopening Anarlog.</Trans>
            }
            checked={floatingBar.value}
            onChange={floatingBar.onChange}
          />
        </>
      )}
    </div>
  );
}
