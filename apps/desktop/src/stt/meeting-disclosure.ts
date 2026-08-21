import { commands as detectCommands } from "@anlg/plugin-detect";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  MEETING_DISCLOSURE_MESSAGE_VERSION,
  type DisclosureAttempt,
} from "./meeting-consent";
import { persistDisclosureAttempt } from "./meeting-consent-store";

export const MEETING_DISCLOSURE_MESSAGE =
  "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so";

const MEETING_DISCLOSURE_MAX_ATTEMPTS = 30;
const MEETING_DISCLOSURE_RETRY_INTERVAL_MS = 1_000;
export const MAX_SENT_MEETING_DISCLOSURE_SESSIONS = 256;
const SLACK_BUNDLE_IDS = new Set([
  "com.slack.Slack",
  "com.tinyspeck.slackmacgap",
]);

type MeetingDisclosureOutcome =
  | { status: "sent" }
  | { status: "notSent"; reason: string }
  | { status: "cancelled" };

type MeetingDisclosureAttemptOutcome =
  | { status: "sent" }
  | { status: "notSent"; reason: unknown }
  | { status: "cancelled" };

type MeetingDisclosureTask = {
  cancelled: boolean;
  restartWhenSettled?: () => boolean;
};

const meetingDisclosureTasks = new Map<string, MeetingDisclosureTask>();
const sentMeetingDisclosureSessionIds = new Set<string>();

function hasSentMeetingDisclosure(sessionId: string) {
  if (!sentMeetingDisclosureSessionIds.has(sessionId)) {
    return false;
  }

  sentMeetingDisclosureSessionIds.delete(sessionId);
  sentMeetingDisclosureSessionIds.add(sessionId);
  return true;
}

function rememberSentMeetingDisclosure(sessionId: string) {
  sentMeetingDisclosureSessionIds.delete(sessionId);
  sentMeetingDisclosureSessionIds.add(sessionId);
  while (
    sentMeetingDisclosureSessionIds.size > MAX_SENT_MEETING_DISCLOSURE_SESSIONS
  ) {
    const oldestSessionId = sentMeetingDisclosureSessionIds
      .values()
      .next().value;
    if (oldestSessionId === undefined) {
      break;
    }
    sentMeetingDisclosureSessionIds.delete(oldestSessionId);
  }
}

async function recordDisclosureAttempt(input: {
  sessionId?: string;
  delivery: DisclosureAttempt["delivery"];
  failureReason?: unknown;
  surface?: string;
}) {
  if (!input.sessionId) {
    return;
  }

  const failureReason =
    input.failureReason instanceof Error
      ? input.failureReason.message
      : input.failureReason
        ? String(input.failureReason)
        : "";

  try {
    await persistDisclosureAttempt({
      id:
        globalThis.crypto?.randomUUID?.() ??
        `disclosure-${Date.now()}-${Math.random()}`,
      sessionId: input.sessionId,
      attemptedAt: new Date().toISOString(),
      platform: "slack_huddle",
      surface: input.surface ?? "huddle",
      messageVersion: MEETING_DISCLOSURE_MESSAGE_VERSION,
      message: MEETING_DISCLOSURE_MESSAGE,
      delivery: input.delivery,
      failureReason,
    });
  } catch (error) {
    console.warn("[listener] failed to persist disclosure attempt", error);
  }
}

function meetingDisclosureFailure(reason: unknown): MeetingDisclosureOutcome {
  const detail = reason instanceof Error ? reason.message : String(reason);
  console.warn("[listener] meeting disclosure was not sent", reason);
  sonnerToast.warning(
    "Recording started, but Anarlog could not post the meeting chat disclosure.",
    { id: "meeting-disclosure-send-failed", duration: Infinity },
  );
  return { status: "notSent", reason: detail };
}

async function attemptMeetingRecordingDisclosure(
  isCancelled: () => boolean,
): Promise<MeetingDisclosureAttemptOutcome> {
  if (isCancelled()) {
    return { status: "cancelled" };
  }

  let micAppsResult: Awaited<
    ReturnType<typeof detectCommands.listMicUsingApplications>
  >;

  try {
    micAppsResult = await detectCommands.listMicUsingApplications();
  } catch (error) {
    return isCancelled()
      ? { status: "cancelled" }
      : { status: "notSent", reason: error };
  }

  if (isCancelled()) {
    return { status: "cancelled" };
  }

  if (micAppsResult.status === "error") {
    return { status: "notSent", reason: micAppsResult.error };
  }

  const micActiveBundleIds = [
    ...new Set(micAppsResult.data.map((app) => app.id.trim()).filter(Boolean)),
  ];
  if (!micActiveBundleIds.some((bundleId) => SLACK_BUNDLE_IDS.has(bundleId))) {
    return {
      status: "notSent",
      reason: "no mic-active Slack app was found",
    };
  }

  if (isCancelled()) {
    return { status: "cancelled" };
  }

  let result: Awaited<ReturnType<typeof detectCommands.sendMeetingChatMessage>>;

  try {
    result = await detectCommands.sendMeetingChatMessage(
      MEETING_DISCLOSURE_MESSAGE,
      micActiveBundleIds,
    );
  } catch (error) {
    return isCancelled()
      ? { status: "cancelled" }
      : { status: "notSent", reason: error };
  }

  if (result.status === "error") {
    return isCancelled()
      ? { status: "cancelled" }
      : { status: "notSent", reason: result.error };
  }

  if (result.data.sent) {
    return { status: "sent" };
  }

  if (isCancelled()) {
    return { status: "cancelled" };
  }

  return {
    status: "notSent",
    reason:
      result.data.warnings.join("; ") || "meeting chat mutation was rejected",
  };
}

export async function sendMeetingRecordingDisclosure({
  sessionId,
  isCancelled = () => false,
  maxAttempts = MEETING_DISCLOSURE_MAX_ATTEMPTS,
  retryIntervalMs = MEETING_DISCLOSURE_RETRY_INTERVAL_MS,
}: {
  sessionId?: string;
  isCancelled?: () => boolean;
  maxAttempts?: number;
  retryIntervalMs?: number;
} = {}): Promise<MeetingDisclosureOutcome> {
  let lastFailureReason: unknown = "meeting chat disclosure was not sent";

  for (let attempt = 0; attempt < Math.max(1, maxAttempts); attempt += 1) {
    const outcome = await attemptMeetingRecordingDisclosure(isCancelled);
    if (outcome.status !== "notSent") {
      await recordDisclosureAttempt({
        sessionId,
        delivery: outcome.status === "sent" ? "sent" : "cancelled",
      });
      return outcome;
    }

    lastFailureReason = outcome.reason;
    if (attempt + 1 < Math.max(1, maxAttempts)) {
      await new Promise<void>((resolve) => {
        setTimeout(resolve, retryIntervalMs);
      });
      if (isCancelled()) {
        await recordDisclosureAttempt({
          sessionId,
          delivery: "cancelled",
        });
        return { status: "cancelled" };
      }
    }
  }

  await recordDisclosureAttempt({
    sessionId,
    delivery: "not_sent",
    failureReason: lastFailureReason,
  });
  return meetingDisclosureFailure(lastFailureReason);
}

export function startMeetingRecordingDisclosure(
  sessionId: string,
  isListening: () => boolean,
) {
  if (hasSentMeetingDisclosure(sessionId)) {
    return;
  }

  const existingTask = meetingDisclosureTasks.get(sessionId);
  if (existingTask) {
    if (existingTask.cancelled) {
      existingTask.restartWhenSettled = isListening;
    }
    return;
  }

  const task: MeetingDisclosureTask = {
    cancelled: false,
  };
  meetingDisclosureTasks.set(sessionId, task);

  void sendMeetingRecordingDisclosure({
    sessionId,
    isCancelled: () => task.cancelled || !isListening(),
  }).then(
    (outcome) => {
      if (meetingDisclosureTasks.get(sessionId) !== task) {
        return;
      }

      const restartWhenSettled = task.restartWhenSettled;
      meetingDisclosureTasks.delete(sessionId);
      if (outcome.status === "sent") {
        rememberSentMeetingDisclosure(sessionId);
      } else if (restartWhenSettled?.()) {
        startMeetingRecordingDisclosure(sessionId, restartWhenSettled);
      }
    },
    () => {
      if (meetingDisclosureTasks.get(sessionId) === task) {
        meetingDisclosureTasks.delete(sessionId);
      }
    },
  );
}

export function cancelMeetingRecordingDisclosure(sessionId: string) {
  const task = meetingDisclosureTasks.get(sessionId);
  if (!task) {
    return;
  }

  task.cancelled = true;
}
