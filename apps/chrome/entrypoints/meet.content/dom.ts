const MAX_PARTICIPANTS = 30;
const MAX_PARTICIPANT_NAME_LENGTH = 80;

export type Participant = {
  name: string;
  is_self: boolean;
};

function byTextPattern(value: string | null | undefined, patterns: string[]) {
  const text = (value || "").toLowerCase();
  return patterns.some((pattern) => text.includes(pattern));
}

export function normalizeParticipantName(
  value: string | null | undefined,
): string | null {
  const name = (value || "").trim().replace(/\s+/g, " ");
  if (!name || name.length > MAX_PARTICIPANT_NAME_LENGTH) {
    return null;
  }

  if (byTextPattern(name, ["microphone", "camera", "pin", "present"])) {
    return null;
  }

  return name;
}

export function isSelfParticipant(name: string) {
  const normalized = name.toLowerCase();
  return (
    normalized === "you" ||
    normalized.endsWith("(you)") ||
    normalized.includes(" (you)")
  );
}

export function getMuteState(root: ParentNode) {
  const muteSelectors = [
    "button[aria-label*='microphone' i]",
    "button[aria-label*='mic' i]",
    "button[data-is-muted]",
  ];

  for (const selector of muteSelectors) {
    const button = root.querySelector(selector);
    if (!button) {
      continue;
    }

    const dataMuted = button.getAttribute("data-is-muted");
    if (dataMuted === "true") {
      return true;
    }
    if (dataMuted === "false") {
      return false;
    }

    const ariaPressed = button.getAttribute("aria-pressed");
    if (ariaPressed === "true") {
      return true;
    }
    if (ariaPressed === "false") {
      return false;
    }

    const label = button.getAttribute("aria-label") || "";
    if (byTextPattern(label, ["turn on microphone", "unmute"])) {
      return true;
    }
    if (byTextPattern(label, ["turn off microphone", "mute"])) {
      return false;
    }
  }

  return false;
}

export function getParticipantNames(root: ParentNode): Participant[] {
  const names = new Map<string, Participant>();

  const participantElements = root.querySelectorAll<HTMLElement>(
    "[data-participant-id]",
  );

  for (const participant of participantElements) {
    const participantId = participant
      .getAttribute("data-participant-id")
      ?.trim();

    const selfNameElement =
      participant.querySelector<HTMLElement>("[data-self-name]");
    const labelElement = participant.querySelector<HTMLElement>(
      "[aria-label], span[dir='auto'], span",
    );

    const rawName =
      selfNameElement?.getAttribute("data-self-name") ||
      labelElement?.getAttribute("aria-label") ||
      labelElement?.textContent ||
      participant.getAttribute("aria-label") ||
      "";

    const name = normalizeParticipantName(rawName);
    if (!name) {
      continue;
    }

    const isSelf = Boolean(selfNameElement) || isSelfParticipant(name);
    const key = participantId || name.toLowerCase();
    const existing = names.get(key);

    if (existing) {
      existing.is_self = existing.is_self || isSelf;
      continue;
    }

    names.set(key, { name, is_self: isSelf });

    if (names.size >= MAX_PARTICIPANTS) {
      break;
    }
  }

  return Array.from(names.values());
}

export function isMeetPageActive(
  location: { hostname: string; pathname: string },
  root: ParentNode,
) {
  if (location.hostname !== "meet.google.com") {
    return false;
  }

  const codePattern = /^\/[a-z]{3}-[a-z]{4}-[a-z]{3}/;
  if (codePattern.test(location.pathname)) {
    return true;
  }

  return Boolean(
    root.querySelector("[role='toolbar'], [aria-label*='Call controls' i]"),
  );
}
