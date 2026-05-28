export const AUDIO_RETENTION_DURATION_MS = {
  none: 0,
  oneDay: 24 * 60 * 60 * 1000,
  threeDays: 3 * 24 * 60 * 60 * 1000,
  oneWeek: 7 * 24 * 60 * 60 * 1000,
  oneMonth: 30 * 24 * 60 * 60 * 1000,
} as const;

export type ExpiringAudioRetentionPolicy =
  keyof typeof AUDIO_RETENTION_DURATION_MS;
export type AudioRetentionPolicy = ExpiringAudioRetentionPolicy | "forever";

const AUDIO_RETENTION_VALUES = new Set([
  ...Object.keys(AUDIO_RETENTION_DURATION_MS),
  "forever",
]);

export function normalizeAudioRetention<
  T extends AudioRetentionPolicy | undefined = "forever",
>(value: unknown, fallback?: T): AudioRetentionPolicy | T {
  if (typeof value === "string" && AUDIO_RETENTION_VALUES.has(value)) {
    return value as AudioRetentionPolicy;
  }

  if (value === false) {
    return "none";
  }

  if (value === true) {
    return "forever";
  }

  return arguments.length >= 2 ? (fallback as T) : ("forever" as T);
}
