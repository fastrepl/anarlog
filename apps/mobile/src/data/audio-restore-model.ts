const AUDIO_EXTENSIONS = new Set([
  "aac",
  "caf",
  "flac",
  "m4a",
  "mp3",
  "mp4",
  "ogg",
  "wav",
  "webm",
]);

export function restoredAudioRelativePath(filename: string): string {
  const extension = filename.split(".").at(-1)?.toLowerCase() ?? "";
  return AUDIO_EXTENSIONS.has(extension)
    ? `restored-audio.${extension}`
    : "restored-audio.bin";
}

export function assertRestoredAudioMatches(
  expected: { sha256: string; sizeBytes: number },
  actual: { sha256: string; sizeBytes: number },
): void {
  assertRestorableAudioMetadata(expected);
  if (
    actual.sizeBytes !== expected.sizeBytes ||
    actual.sha256 !== expected.sha256
  ) {
    throw new Error("That file does not match this meeting recording.");
  }
}

export function assertRestorableAudioMetadata(expected: {
  sha256: string;
  sizeBytes: number;
}): void {
  if (!/^[0-9a-f]{64}$/.test(expected.sha256)) {
    throw new Error(
      "This older recording cannot be verified on mobile. Open it on the device that recorded it.",
    );
  }
  if (!Number.isSafeInteger(expected.sizeBytes) || expected.sizeBytes <= 0) {
    throw new Error("This recording has invalid synced file metadata.");
  }
}
