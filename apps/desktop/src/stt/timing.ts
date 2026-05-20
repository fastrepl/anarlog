export type TranscriptTimingSource =
  | "provider_word"
  | "provider_segment_interpolated"
  | "synthetic_text";

export type TranscriptWordMetadata = Record<string, unknown>;

export function createTranscriptTimingMetadata(
  source: TranscriptTimingSource,
  metadata?: unknown,
): TranscriptWordMetadata {
  const base = isRecord(metadata) ? metadata : {};
  const timing = isRecord(base.timing) ? base.timing : {};

  return {
    ...base,
    timing: {
      ...timing,
      source,
    },
  };
}

export function getTranscriptTimingSource(word: {
  metadata?: unknown;
}): TranscriptTimingSource {
  const metadata = word.metadata;
  if (!isRecord(metadata)) {
    return "provider_word";
  }

  const timing = metadata.timing;
  if (!isRecord(timing)) {
    return getValidTimingSource(metadata.timing_source) ?? "provider_word";
  }

  return (
    getValidTimingSource(timing.source) ??
    getValidTimingSource(metadata.timing_source) ??
    "provider_word"
  );
}

export function isTranscriptWordSeekable(word: { metadata?: unknown }) {
  return getTranscriptTimingSource(word) !== "synthetic_text";
}

export function getValidTimingSource(
  source: unknown,
): TranscriptTimingSource | undefined {
  return source === "provider_word" ||
    source === "provider_segment_interpolated" ||
    source === "synthetic_text"
    ? source
    : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
