import {
  getStoredSettingValues,
  type StoredSettingValues,
} from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";

export type NotificationSettingKey =
  | "notification_cloudsync_complete"
  | "notification_summary_complete"
  | "notification_transcription_complete";

export async function shouldShowNotification(
  settingKey: NotificationSettingKey,
): Promise<boolean> {
  const stored = await getStoredSettingValues();
  return shouldShowNotificationFromSettings(stored, settingKey);
}

export function shouldShowNotificationFromSettings(
  stored: StoredSettingValues,
  settingKey: NotificationSettingKey,
): boolean {
  return (
    !resolveConfigValue("notification_disabled", stored) &&
    resolveConfigValue(settingKey, stored)
  );
}
