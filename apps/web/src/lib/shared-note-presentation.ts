import type { SharedNoteAttachment } from "@/lib/shared-notes";

const MAX_VISIBLE_PARTICIPANT_AVATARS = 5;

const RELATIVE_TIME_DIVISIONS: Array<
  [amount: number, unit: Intl.RelativeTimeFormatUnit]
> = [
  [60, "second"],
  [60, "minute"],
  [24, "hour"],
  [7, "day"],
  [4.34524, "week"],
  [12, "month"],
  [Number.POSITIVE_INFINITY, "year"],
];

export function findFeaturedSharedNoteAudio(
  attachments: readonly SharedNoteAttachment[],
) {
  return attachments.find((attachment) =>
    attachment.contentType.startsWith("audio/"),
  );
}

export function createSharedNoteParticipantPresentation(
  participants: readonly string[],
) {
  const normalizedParticipants = Array.from(
    new Set(
      participants
        .map((participant) => participant.replace(/\s+/g, " ").trim())
        .filter(Boolean),
    ),
  );
  const visibleNames = normalizedParticipants
    .slice(0, 2)
    .map((participant) => participant.split(/\s+/)[0]);
  const nameOverflow = normalizedParticipants.length - visibleNames.length;

  return {
    avatarParticipants: normalizedParticipants.slice(
      0,
      MAX_VISIBLE_PARTICIPANT_AVATARS,
    ),
    label: normalizedParticipants.length
      ? `${visibleNames.join(", ")}${nameOverflow > 0 ? ` +${nameOverflow} more` : ""}`
      : "No participants listed",
    participantCount: normalizedParticipants.length,
  };
}

export function formatSharedNoteMeetingAt(value: string) {
  return new Intl.DateTimeFormat("en-US", {
    day: "numeric",
    month: "long",
    timeZone: "UTC",
    year: "numeric",
  }).format(new Date(value));
}

export function createSharedNoteWaveform(seed: string, count = 96) {
  let state = 2_166_136_261;
  for (const character of seed) {
    state ^= character.charCodeAt(0);
    state = Math.imul(state, 16_777_619);
  }

  return Array.from({ length: count }, (_, index) => {
    state ^= index + 1;
    state = Math.imul(state, 2_246_822_519);
    state ^= state >>> 13;
    return 18 + (Math.abs(state) % 73);
  });
}

export function formatSharedNotePlaybackTime(value: number) {
  if (!Number.isFinite(value) || value < 0) return "0:00";
  const seconds = Math.floor(value);
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
}

export function formatSharedNoteRelativeTime(value: string, now = Date.now()) {
  const formatter = new Intl.RelativeTimeFormat("en-US", { numeric: "auto" });
  let duration = (Date.parse(value) - now) / 1000;
  for (const [amount, unit] of RELATIVE_TIME_DIVISIONS) {
    if (Math.abs(duration) < amount) {
      return formatter.format(Math.round(duration), unit);
    }
    duration /= amount;
  }
  return "";
}

export function isSharedNoteAudioGrantExpiring(
  expiresAt: string,
  now = Date.now(),
) {
  return Date.parse(expiresAt) - now <= 10_000;
}

export function formatSharedNotePublishedAt(value: string) {
  return new Intl.DateTimeFormat("en-US", {
    day: "numeric",
    month: "short",
    timeZone: "UTC",
    year: "numeric",
  }).format(new Date(value));
}
