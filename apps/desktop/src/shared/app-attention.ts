import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";

import { resolveConfigValue } from "./config";
import { isAppWindowInactive } from "./window-activity";

import { getStoredSettingValues } from "~/settings/queries";

export type AppAttentionEvent = "summary_ready" | "transcript_ready";

const ATTENTION_SETTING_KEY = {
  summary_ready: "notification_bounce_summary",
  transcript_ready: "notification_bounce_transcript",
} as const;

export async function requestAppAttention(event: AppAttentionEvent) {
  try {
    const stored = await getStoredSettingValues();
    if (!resolveConfigValue(ATTENTION_SETTING_KEY[event], stored)) {
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
