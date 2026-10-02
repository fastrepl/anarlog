import {
  getSummaryEligibility,
  type SummaryEligibility,
  type SummaryEligibilitySkipCode,
} from "@anlg/utils/summary-eligibility";

import { countTranscriptWordCharacters } from "./summary-length";

export type EnhanceEligibilitySkipCode = SummaryEligibilitySkipCode;

export function getEligibility(
  transcripts: ReadonlyArray<{
    words: ReadonlyArray<{ text?: unknown }>;
  }>,
): SummaryEligibility {
  return getSummaryEligibility({
    transcriptCount: transcripts.length,
    wordCount: transcripts.reduce(
      (total, transcript) => total + transcript.words.length,
      0,
    ),
    characterCount: countTranscriptWordCharacters(transcripts),
  });
}
