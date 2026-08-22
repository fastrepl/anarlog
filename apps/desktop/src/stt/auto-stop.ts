import { commands as notificationCommands } from "@anlg/plugin-notification";

import {
  AUTO_STOP_CONFIRM_TIMEOUT_SECONDS,
  createAutoStopEndedNotificationKey,
} from "./auto-stop-notification";
import {
  BROWSER_AUTO_STOP_APP_IDS,
  getNotificationIconForApp,
} from "./meeting-apps";

import { loadSessionEvent } from "~/session/queries";

export const AUTO_STOP_CONFIRM_DELAY_MS = 5_000;
export const AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS = 3 * 60_000;
export const AUTO_STOP_CALENDAR_EARLY_START_BUFFER_MS = 5 * 60_000;
export const AUTO_STOP_EVENT_END_GRACE_MS = 10 * 60_000;
export const AUTO_STOP_NETWORK_HOLD_MS = 8 * 60_000;
export const AUTO_STOP_RECENT_OFFLINE_MS = 60_000;

const UNRELIABLE_AUTO_STOP_APP_IDS = new Set(["com.kakao.KakaoTalkMac"]);

function parseEventTimeMs(value: string | undefined): number | null {
  if (!value) {
    return null;
  }

  const ms = new Date(value).getTime();
  return Number.isNaN(ms) ? null : ms;
}

export async function shouldPromptBeforeAutoStopping({
  appIds,
  sessionId,
  nowMs,
}: {
  appIds: string[];
  sessionId: string | null;
  nowMs: number;
}): Promise<boolean> {
  if (!appIds.some((id) => BROWSER_AUTO_STOP_APP_IDS.has(id))) {
    return false;
  }

  if (!sessionId) {
    return false;
  }

  const event = await loadSessionEvent(sessionId);
  if (!event || event.is_all_day) {
    return false;
  }

  const endMs = parseEventTimeMs(event.ended_at);
  if (!endMs) {
    return false;
  }

  const startMs = parseEventTimeMs(event.started_at);
  if (startMs && nowMs < startMs - AUTO_STOP_CALENDAR_EARLY_START_BUFFER_MS) {
    return false;
  }

  return nowMs < endMs - AUTO_STOP_CALENDAR_EARLY_END_THRESHOLD_MS;
}

export async function getNetworkInterruptionDeadlineMs({
  sessionId,
  nowMs,
}: {
  sessionId: string | null;
  nowMs: number;
}): Promise<number | null> {
  if (!sessionId) {
    return null;
  }

  const event = await loadSessionEvent(sessionId);
  if (!event || event.is_all_day) {
    return null;
  }

  const endMs = parseEventTimeMs(event.ended_at);
  if (!endMs) {
    return null;
  }

  const startMs = parseEventTimeMs(event.started_at);
  if (startMs && nowMs < startMs - AUTO_STOP_CALENDAR_EARLY_START_BUFFER_MS) {
    return null;
  }

  const deadlineMs = endMs + AUTO_STOP_EVENT_END_GRACE_MS;
  return deadlineMs > nowMs ? deadlineMs : null;
}

export function resolveNetworkHoldUntilMs({
  calendarDeadlineMs,
  nowMs,
}: {
  calendarDeadlineMs: number | null;
  nowMs: number;
}) {
  if (calendarDeadlineMs != null && calendarDeadlineMs > nowMs) {
    return calendarDeadlineMs;
  }

  return nowMs + AUTO_STOP_NETWORK_HOLD_MS;
}

export function isRecentNetworkDrop(
  lastReconnectAtMs: number | null,
  nowMs: number,
) {
  return (
    lastReconnectAtMs != null &&
    nowMs - lastReconnectAtMs <= AUTO_STOP_RECENT_OFFLINE_MS
  );
}

function getPrimaryStoppedApp(
  stoppedTriggerAppIds: string[],
  stoppedApps: { id: string; name: string }[],
) {
  return (
    stoppedApps.find(
      (app) =>
        stoppedTriggerAppIds.includes(app.id) &&
        BROWSER_AUTO_STOP_APP_IDS.has(app.id),
    ) ??
    stoppedApps.find((app) => stoppedTriggerAppIds.includes(app.id)) ??
    null
  );
}

export function getAutoStopCandidateAppIds(
  triggerAppIds: string[] | null | undefined,
  stoppedApps: { id: string }[],
) {
  const trigger = triggerAppIds ?? [];
  const stoppedIds = new Set(stoppedApps.map((app) => app.id));
  const stoppedTriggerAppIds = trigger.filter((id) => stoppedIds.has(id));
  const candidateAppIds =
    stoppedTriggerAppIds.length > 0 ? stoppedTriggerAppIds : trigger;

  return candidateAppIds.filter((id) => !UNRELIABLE_AUTO_STOP_APP_IDS.has(id));
}

export function getAutoStopActiveCheckAppIds(
  triggerAppIds: string[] | null | undefined,
  candidateAppIds: string[],
) {
  const unreliableTriggerAppIds =
    triggerAppIds?.filter((id) => UNRELIABLE_AUTO_STOP_APP_IDS.has(id)) ?? [];

  return [...new Set([...candidateAppIds, ...unreliableTriggerAppIds])];
}

export async function showMeetingEndedPrompt({
  sessionId,
  stoppedTriggerAppIds,
  stoppedApps,
}: {
  sessionId: string;
  stoppedTriggerAppIds: string[];
  stoppedApps: { id: string; name: string }[];
}) {
  const app = getPrimaryStoppedApp(stoppedTriggerAppIds, stoppedApps);

  await notificationCommands.showNotification({
    key: createAutoStopEndedNotificationKey(sessionId),
    title: "Did your meeting end?",
    message: `Anarlog will stop listening in ${AUTO_STOP_CONFIRM_TIMEOUT_SECONDS} seconds.`,
    timeout: { secs: AUTO_STOP_CONFIRM_TIMEOUT_SECONDS, nanos: 0 },
    source: null,
    start_time: null,
    participants: null,
    event_details: null,
    action_label: "Stop",
    action_variant: "destructive",
    options: null,
    footer: null,
    icon: app ? await getNotificationIconForApp(app) : null,
  });
}
