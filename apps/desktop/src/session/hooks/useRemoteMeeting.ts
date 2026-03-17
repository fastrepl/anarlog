import { useSessionEvent } from "~/store/tinybase/hooks";

export type RemoteMeeting = {
  type: "zoom" | "google-meet" | "webex" | "teams";
  url: string;
};

export function detectMeetingType(
  url: string,
): "zoom" | "google-meet" | "webex" | "teams" | null {
  try {
    const parsed = new URL(url);
    const hostname = parsed.hostname.toLowerCase();

    if (hostname.includes("zoom.us")) {
      return "zoom";
    }
    if (hostname.includes("meet.google.com")) {
      return "google-meet";
    }
    if (hostname.includes("webex.com")) {
      return "webex";
    }
    if (hostname.includes("teams.microsoft.com")) {
      return "teams";
    }
    return null;
  } catch {
    return null;
  }
}

export function useRemoteMeeting(sessionId: string): RemoteMeeting | null {
  const event = useSessionEvent(sessionId);
  const meetingLink = event?.meeting_link ?? null;

  if (!meetingLink) {
    return null;
  }

  const type = detectMeetingType(meetingLink);
  if (!type) {
    return null;
  }

  return { type, url: meetingLink };
}
