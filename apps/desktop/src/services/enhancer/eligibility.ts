import type * as main from "~/store/tinybase/store/main";

export const MIN_WORDS_FOR_ENHANCEMENT = 5;

export function countTranscriptWords(
  transcriptIds: string[],
  store: main.Store | undefined,
): number {
  if (!store) return 0;

  let totalWordCount = 0;
  for (const transcriptId of transcriptIds) {
    const wordsJson = store.getCell("transcripts", transcriptId, "words") as
      | string
      | undefined;
    if (wordsJson) {
      totalWordCount += (JSON.parse(wordsJson) as unknown[]).length;
    }
  }
  return totalWordCount;
}

type EligibilityResult =
  | { eligible: true; wordCount: number }
  | { eligible: false; reason: string; wordCount: number };

export function getEligibility(
  hasTranscript: boolean,
  transcriptIds: string[],
  store: main.Store | undefined,
): EligibilityResult {
  if (!hasTranscript) {
    return { eligible: false, reason: "No transcript recorded", wordCount: 0 };
  }

  const wordCount = countTranscriptWords(transcriptIds, store);

  if (wordCount < MIN_WORDS_FOR_ENHANCEMENT) {
    return {
      eligible: false,
      reason: `Not enough words recorded (${wordCount}/${MIN_WORDS_FOR_ENHANCEMENT} minimum)`,
      wordCount,
    };
  }

  return { eligible: true, wordCount };
}
