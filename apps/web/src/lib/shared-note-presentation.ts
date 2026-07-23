import type { SharedNoteAttachment } from "@/lib/shared-notes";

export function findFeaturedSharedNoteAudio(
  attachments: readonly SharedNoteAttachment[],
) {
  return attachments.find((attachment) =>
    attachment.contentType.startsWith("audio/"),
  );
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

export function formatSharedNotePublishedAt(value: string) {
  return new Intl.DateTimeFormat("en-US", {
    day: "numeric",
    month: "short",
    timeZone: "UTC",
    year: "numeric",
  }).format(new Date(value));
}
