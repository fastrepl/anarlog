import { t } from "@lingui/core/macro";

import { commands as notificationCommands } from "@anlg/plugin-notification";

import { isAppWindowInactive } from "~/shared/window-activity";

const SUMMARY_READY_NOTIFICATION_TIMEOUT_SECONDS = 15;

export const SUMMARY_READY_NOTIFICATION_KEY_PREFIX = "summary-ready:" as const;

export function createSummaryReadyNotificationKey(sessionId: string) {
  return `${SUMMARY_READY_NOTIFICATION_KEY_PREFIX}${sessionId}:${crypto.randomUUID()}`;
}

export async function showSummaryReadyNotification(
  sessionId: string,
  sessionTitle?: string,
) {
  if (!(await isAppWindowInactive())) {
    return;
  }

  const title = sessionTitle?.trim();

  try {
    const result = await notificationCommands.showNotification({
      key: createSummaryReadyNotificationKey(sessionId),
      title: t`Summary ready`,
      message: title
        ? t`"${title}" is ready to read.`
        : t`Your summary is ready.`,
      timeout: {
        secs: SUMMARY_READY_NOTIFICATION_TIMEOUT_SECONDS,
        nanos: 0,
      },
      source: { type: "session", session_id: sessionId },
      start_time: null,
      participants: null,
      event_details: null,
      action_label: t`Open Anarlog`,
      action_variant: null,
      options: null,
      footer: null,
      icon: null,
    });

    if (result.status === "error") {
      console.error(
        "[enhance] failed to show summary-ready notification",
        result.error,
      );
    }
  } catch (error) {
    console.error("[enhance] failed to show summary-ready notification", error);
  }
}
