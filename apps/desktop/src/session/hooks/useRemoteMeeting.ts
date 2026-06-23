import { useSessionEvent } from "~/store/tinybase/hooks";

export type RemoteMeetingType =
  | "zoom"
  | "google-meet"
  | "webex"
  | "teams"
  | "cal-com";

export type RemoteMeeting = {
  type: RemoteMeetingType;
  url: string;
};

export function detectMeetingType(url: string): RemoteMeetingType | null {
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
    if (hostname === "app.cal.com" && parsed.pathname.startsWith("/video/")) {
      return "cal-com";
    }
    return null;
  } catch {
    return null;
  }
}

export function getRemoteMeeting(
  meetingLink: string | null | undefined,
): RemoteMeeting | null {
  if (!meetingLink) {
    return null;
  }

  const type = detectMeetingType(meetingLink);
  if (!type) {
    return null;
  }

  return { type, url: meetingLink };
}

export function useRemoteMeeting(sessionId: string): RemoteMeeting | null {
  const event = useSessionEvent(sessionId);
  return getRemoteMeeting(event?.meeting_link);
}
