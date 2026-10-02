export const MIN_WORDS_FOR_SUMMARY = 5;
export const MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY = 160;

export type SummaryEligibilitySkipCode =
  | "no_transcript"
  | "transcript_too_short";

export type SummaryEligibility =
  | { eligible: true; characterCount: number; wordCount: number }
  | {
      eligible: false;
      code: SummaryEligibilitySkipCode;
      characterCount: number;
      reason: string;
      wordCount: number;
    };

export function getSummaryEligibility({
  transcriptCount,
  wordCount,
  characterCount,
}: {
  transcriptCount: number;
  wordCount: number;
  characterCount: number;
}): SummaryEligibility {
  if (transcriptCount === 0) {
    return {
      eligible: false,
      code: "no_transcript",
      reason: "No transcript recorded",
      characterCount: 0,
      wordCount: 0,
    };
  }

  if (wordCount < MIN_WORDS_FOR_SUMMARY) {
    return {
      eligible: false,
      code: "transcript_too_short",
      reason: `Not enough words recorded (${wordCount}/${MIN_WORDS_FOR_SUMMARY} minimum)`,
      characterCount,
      wordCount,
    };
  }

  if (characterCount < MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY) {
    return {
      eligible: false,
      code: "transcript_too_short",
      reason: `Transcript too short to summarize (${characterCount}/${MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY} characters minimum)`,
      characterCount,
      wordCount,
    };
  }

  return { eligible: true, characterCount, wordCount };
}
