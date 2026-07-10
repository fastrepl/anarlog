export const MIN_WORDS_FOR_ENHANCEMENT = 5;

export function countTranscriptWords(
  transcripts: ReadonlyArray<{ words: readonly unknown[] }>,
): number {
  return transcripts.reduce(
    (total, transcript) => total + transcript.words.length,
    0,
  );
}

type EligibilityResult =
  | { eligible: true; wordCount: number }
  | { eligible: false; reason: string; wordCount: number };

export function getEligibility(
  transcripts: ReadonlyArray<{ words: readonly unknown[] }>,
): EligibilityResult {
  if (transcripts.length === 0) {
    return { eligible: false, reason: "No transcript recorded", wordCount: 0 };
  }

  const wordCount = countTranscriptWords(transcripts);

  if (wordCount < MIN_WORDS_FOR_ENHANCEMENT) {
    return {
      eligible: false,
      reason: `Not enough words recorded (${wordCount}/${MIN_WORDS_FOR_ENHANCEMENT} minimum)`,
      wordCount,
    };
  }

  return { eligible: true, wordCount };
}
