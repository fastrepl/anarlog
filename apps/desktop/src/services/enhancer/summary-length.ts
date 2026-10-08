import type { SummaryLengthPolicy as RustSummaryLengthPolicy } from "@anlg/plugin-template";

export type SummaryLengthPolicy = RustSummaryLengthPolicy;

export function countNormalizedCharacters(text: string): number {
  return Array.from(text.replace(/\s+/gu, " ").trim()).length;
}

export function countTranscriptWordCharacters(
  transcripts: ReadonlyArray<{
    words: ReadonlyArray<{ text?: unknown }>;
  }>,
): number {
  return countNormalizedCharacters(
    transcripts
      .flatMap((transcript) => transcript.words)
      .map((word) => (typeof word.text === "string" ? word.text : ""))
      .filter(Boolean)
      .join(" "),
  );
}

export function formatSummaryDetailGuidance(
  hasTemplateSections: boolean,
): string {
  return [
    "Summary mode: detailed. Capture every material topic, decision, rationale, example, open question, and commitment.",
    "Explain material points with concrete details and enough context to stand on their own.",
    "Retain useful secondary discussion and examples, but remove repetition and conversational filler.",
    hasTemplateSections
      ? "Preserve every requested template section and do not add sections based on this mode."
      : "Follow the requested format and include only explicitly stated or unambiguous owners, commitments, and deadlines; do not turn proposals into commitments.",
  ].join(" ");
}

export function formatSummaryLengthGuidance(
  policy: SummaryLengthPolicy | null,
  options: { customFormat?: boolean; hasTemplateSections?: boolean } = {},
): string | null {
  const guidance = policy?.guidance;
  if (!policy || !guidance) {
    return null;
  }

  const { customFormat = false, hasTemplateSections = false } = options;

  const sections =
    guidance.min_sections === guidance.max_sections
      ? `exactly ${guidance.max_sections} section${guidance.max_sections === 1 ? "" : "s"}`
      : `${guidance.min_sections} to ${guidance.max_sections} sections`;

  return [
    `Summary length: the transcript contains about ${policy.transcript_characters} characters.`,
    hasTemplateSections
      ? `Keep every requested template section and stay under ${guidance.max_characters} characters overall.`
      : customFormat
        ? `Keep the requested structure and stay under ${guidance.max_characters} characters overall.`
        : `Keep the summary proportional to it: use ${sections} and stay under ${guidance.max_characters} characters overall.`,
    "A short meeting must produce a short summary; never pad with filler.",
  ].join(" ");
}
