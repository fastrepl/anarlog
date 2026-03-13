const MAX_URL_LENGTH = 2048;
const MAX_PARTICIPANTS = 30;
const MAX_PARTICIPANT_NAME_LENGTH = 80;

type Participant = {
  name: string;
  is_self: boolean;
};

type MeetingStateMessage = {
  type: "meeting_state";
  url: string;
  is_active: true;
  muted: boolean;
  participants: Participant[];
};

type MeetingEndedMessage = {
  type: "meeting_ended";
  url: string;
  is_active: false;
};

export type HostMessage = MeetingStateMessage | MeetingEndedMessage;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isMeetUrl(value: unknown): value is string {
  if (typeof value !== "string") {
    return false;
  }

  const url = value.trim();
  if (!url || url.length > MAX_URL_LENGTH) {
    return false;
  }

  try {
    const parsed = new URL(url);
    return (
      parsed.protocol === "https:" && parsed.hostname === "meet.google.com"
    );
  } catch {
    return false;
  }
}

function parseParticipant(value: unknown): Participant | null {
  if (!isRecord(value)) {
    return null;
  }

  const name = typeof value.name === "string" ? value.name.trim() : "";
  if (!name || name.length > MAX_PARTICIPANT_NAME_LENGTH) {
    return null;
  }

  if (typeof value.is_self !== "boolean") {
    return null;
  }

  return {
    name,
    is_self: value.is_self,
  };
}

export function parseIncomingMessage(message: unknown): HostMessage | null {
  if (!isRecord(message) || typeof message.type !== "string") {
    return null;
  }

  if (message.type === "meeting_state") {
    if (message.is_active !== true || typeof message.muted !== "boolean") {
      return null;
    }

    const { url, participants: rawParticipants } = message;
    if (!isMeetUrl(url) || !Array.isArray(rawParticipants)) {
      return null;
    }

    const participants: Participant[] = [];
    for (const rawParticipant of rawParticipants) {
      const participant = parseParticipant(rawParticipant);
      if (!participant) {
        continue;
      }

      participants.push(participant);
      if (participants.length >= MAX_PARTICIPANTS) {
        break;
      }
    }

    return {
      type: "meeting_state",
      url,
      is_active: true,
      muted: message.muted,
      participants,
    };
  }

  if (message.type === "meeting_ended") {
    const { url } = message;
    if (message.is_active !== false || !isMeetUrl(url)) {
      return null;
    }

    return {
      type: "meeting_ended",
      url,
      is_active: false,
    };
  }

  return null;
}
