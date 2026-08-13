import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";

import { resolveConfigValue } from "./config";
import { isAppWindowInactive } from "./window-activity";

import { getStoredSettingValues } from "~/settings/queries";

export async function requestAppAttention() {
  try {
    const stored = await getStoredSettingValues();
    if (!resolveConfigValue("notification_bounce", stored)) {
      return;
    }
    if (!resolveConfigValue("show_app_in_dock", stored)) {
      return;
    }

    if (!(await isAppWindowInactive())) {
      return;
    }

    await getCurrentWindow().requestUserAttention(
      UserAttentionType.Informational,
    );
  } catch (error) {
    console.error("[app-attention] failed to request user attention", error);
  }
}
