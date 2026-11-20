export type ParticipantConsentStatus = "unknown" | "consented" | "declined";

type ParticipantConsentSource =
  | "explicit_chat_reply"
  | "explicit_ui"
  | "unseen";

export type ParticipantConsent = {
  sessionId: string;
  participantKey: string;
  status: ParticipantConsentStatus;
  source: ParticipantConsentSource;
  updatedAt: string;
};

export type SessionListeningPolicy = "continue" | "stop_declined";

const DECLINE_PATTERN =
  /\b((i\s+)?(do\s+not|don't|does\s+not|doesn't)\s+consent|stop\s+recording)\b/i;
const CONSENT_PATTERN = /\b(i\s+consent|i\s+agree\s+to\s+(being\s+)?record)/i;

export function interpretChatAsConsentResponse(
  text: string,
  disclosureMessage: string,
): ParticipantConsentStatus | null {
  const normalized = text.replace(/\s+/g, " ").trim();
  if (!normalized) {
    return null;
  }
  if (normalized === disclosureMessage.replace(/\s+/g, " ").trim()) {
    return null;
  }
  if (DECLINE_PATTERN.test(normalized)) {
    return "declined";
  }
  if (CONSENT_PATTERN.test(normalized)) {
    return "consented";
  }
  return null;
}

export function sessionListeningPolicy(
  consents: readonly ParticipantConsent[],
): SessionListeningPolicy {
  return consents.some((consent) => consent.status === "declined")
    ? "stop_declined"
    : "continue";
}
