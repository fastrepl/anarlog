import { Trans } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";

import { DefaultMeetingShareAccessSelector } from "./default-share-access";
import { PrimaryRecordingDeviceSelector } from "./primary-recording-device";

import { useBillingAccess } from "~/auth/billing-context";
import { PlanGate } from "~/settings/plan-gate";
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
  captureSharedScreens,
}: {
  autoJoinScheduledMeetings: SettingItem;
  autoStartScheduledMeetings: SettingItem;
  autoStopMeetings: SettingItem;
  floatingBar: SettingItem;
  meetingDisclosureAutoPost: SettingItem;
  captureMeetingChat: SettingItem;
  captureSharedScreens: SettingItem;
}) {
  const { isPro } = useBillingAccess();
  const currentPlatform = platform();
  const supportsMeetingAx =
    currentPlatform === "macos" || currentPlatform === "linux";
  const supportsMicDetection = currentPlatform !== "windows";
  const supportsScreenCapture =
    currentPlatform === "macos" || currentPlatform === "windows";

  return (
    <div className="flex flex-col gap-4">
      <DefaultMeetingShareAccessSelector />
      <PrimaryRecordingDeviceSelector />
      <PlanGate plan="pro" allowed={isPro} className="flex flex-col gap-4">
        <SettingSwitchRow
          title={<Trans>Start when meeting begins</Trans>}
          checked={isPro && autoStartScheduledMeetings.value}
          onChange={autoStartScheduledMeetings.onChange}
        />
        <SettingSwitchRow
          title={<Trans>Join scheduled meetings</Trans>}
          checked={isPro && autoJoinScheduledMeetings.value}
          onChange={autoJoinScheduledMeetings.onChange}
          disabled={!autoStartScheduledMeetings.value}
        />
      </PlanGate>
      {supportsMicDetection && (
        <SettingSwitchRow
          title={<Trans>Stop when meeting ends</Trans>}
          checked={autoStopMeetings.value}
          onChange={autoStopMeetings.onChange}
        />
      )}
      {supportsMeetingAx && (
        <>
          <SettingSwitchRow
            title={<Trans>Post recording disclosure in meeting chat</Trans>}
            checked={meetingDisclosureAutoPost.value}
            onChange={meetingDisclosureAutoPost.onChange}
          />
          <SettingSwitchRow
            title={<Trans>Capture meeting chat in Memos</Trans>}
            checked={captureMeetingChat.value}
            onChange={captureMeetingChat.onChange}
          />
        </>
      )}
      {supportsScreenCapture && (
        <SettingSwitchRow
          title={<Trans>Capture shared screens in Memos</Trans>}
          checked={captureSharedScreens.value}
          onChange={captureSharedScreens.onChange}
        />
      )}
      <SettingSwitchRow
        title={<Trans>Show floating bar</Trans>}
        checked={floatingBar.value}
        onChange={floatingBar.onChange}
      />
    </div>
  );
}
