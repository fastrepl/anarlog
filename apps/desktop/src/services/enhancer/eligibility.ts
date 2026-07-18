import {
  countTranscriptWordCharacters,
  MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY,
} from "./summary-length";

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
  | { eligible: true; characterCount: number; wordCount: number }
  | {
      eligible: false;
      characterCount: number;
      reason: string;
      wordCount: number;
    };

export function getEligibility(
  transcripts: ReadonlyArray<{
    words: ReadonlyArray<{ text?: unknown }>;
  }>,
): EligibilityResult {
  if (transcripts.length === 0) {
    return {
      eligible: false,
      reason: "No transcript recorded",
      characterCount: 0,
      wordCount: 0,
    };
  }

  const wordCount = countTranscriptWords(transcripts);
  const characterCount = countTranscriptWordCharacters(transcripts);

  if (wordCount < MIN_WORDS_FOR_ENHANCEMENT) {
    return {
      eligible: false,
      reason: `Not enough words recorded (${wordCount}/${MIN_WORDS_FOR_ENHANCEMENT} minimum)`,
      characterCount,
      wordCount,
    };
  }

  if (characterCount < MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY) {
    return {
      eligible: false,
      reason: `Transcript too short to summarize (${characterCount}/${MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY} characters minimum)`,
      characterCount,
      wordCount,
    };
  }

  return { eligible: true, characterCount, wordCount };
}
