export type RemoteMeetingType =
  | "zoom"
  | "google-meet"
  | "webex"
  | "teams"
  | "cal-com"
  | "x-call";

export type RemoteMeeting = {
  type: RemoteMeetingType;
  url: string;
};

function isXCallUrl(parsed: URL): boolean {
  const hostname = parsed.hostname.toLowerCase();
  const segments = parsed.pathname.split("/").filter(Boolean);

  if (hostname === "call.x.com") {
    return segments.length === 1 && segments[0] !== "new";
  }

  if (hostname === "x.com" || hostname === "www.x.com") {
    return (
      (segments.length === 3 &&
        segments[0] === "i" &&
        segments[1] === "call") ||
      (segments.length === 2 && segments[0] === "call")
    );
  }

  return false;
}

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
    if (isXCallUrl(parsed)) {
      return "x-call";
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
