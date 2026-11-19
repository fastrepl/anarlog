import {
  events as detectEvents,
  type MeetingNoticeEvent,
} from "@anlg/plugin-detect";
import { toast } from "@anlg/ui/components/ui/toast";

/** The disclosure the meeting notice posts; chat capture ignores it as a consent reply. */
export const MEETING_DISCLOSURE_MESSAGE =
  "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so";

const MEETING_DISCLOSURE_TOAST_ID = "meeting-disclosure-send-failed";

const watches = new Map<string, () => void>();

function showMeetingNoticeOutcome(event: MeetingNoticeEvent) {
  if (event.outcome === "notSent") {
    toast.warning(
      "Recording started, but Anarlog could not post the meeting chat disclosure.",
      {
        id: MEETING_DISCLOSURE_TOAST_ID,
        duration: Infinity,
        description:
          event.reason === "permission_required"
            ? "Allow Accessibility for Anarlog to post it."
            : undefined,
      },
    );
  } else if (event.outcome === "unconfirmed") {
    toast.warning("Anarlog may have posted the meeting chat disclosure.", {
      id: MEETING_DISCLOSURE_TOAST_ID,
      duration: Infinity,
      description: "It wasn't confirmed, so it won't be posted again.",
    });
  }
}

/**
 * Tells the person how the recording's meeting chat disclosure ended. Rust posts it at most once
 * per session for every recording; this only listens for its outcome.
 */
export function watchMeetingNotice(
  sessionId: string,
  isListening: () => boolean,
) {
  stopWatchingMeetingNotice(sessionId);
  let unlisten: (() => void) | undefined;
  let stopped = false;
  const stop = () => {
    stopped = true;
    unlisten?.();
  };
  watches.set(sessionId, stop);

  detectEvents.meetingNoticeEvent
    .listen(({ payload }) => {
      if (stopped || payload.sessionId !== sessionId) {
        return;
      }
      if (watches.get(sessionId) === stop) {
        watches.delete(sessionId);
      }
      stop();
      if (isListening()) {
        showMeetingNoticeOutcome(payload);
      }
    })
    .then(
      (listener) => {
        if (stopped) {
          listener();
        } else {
          unlisten = listener;
        }
      },
      (error: unknown) => {
        console.warn(
          "[listener] failed to watch the meeting chat notice",
          error,
        );
      },
    );
}

export function stopWatchingMeetingNotice(sessionId: string) {
  const stop = watches.get(sessionId);
  if (!stop) {
    return;
  }
  watches.delete(sessionId);
  stop();
}
