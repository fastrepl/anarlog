import type {
  SummaryLengthMode as RustSummaryLengthMode,
  SummaryLengthPolicy as RustSummaryLengthPolicy,
} from "@anlg/plugin-template";

const SUMMARY_LENGTH_MODES: SummaryLengthMode[] = [
  "crisp",
  "balanced",
  "detailed",
];
const DEFAULT_SUMMARY_LENGTH_MODE: SummaryLengthMode = "detailed";

export type SummaryLengthMode = RustSummaryLengthMode;
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

export function normalizeSummaryLengthMode(value: unknown): SummaryLengthMode {
  return SUMMARY_LENGTH_MODES.includes(value as SummaryLengthMode)
    ? (value as SummaryLengthMode)
    : DEFAULT_SUMMARY_LENGTH_MODE;
}

function getRelativeLengthDescription(mode: SummaryLengthMode): string {
  if (mode === "crisp") {
    return "about half the length of a balanced summary";
  }
  if (mode === "balanced") {
    return "the baseline length";
  }
  return "about twice the length of a balanced summary";
}

export function formatSummaryLengthModeGuidance(
  mode: SummaryLengthMode,
  hasTemplateSections: boolean,
): string {
  const templateGuidance = hasTemplateSections
    ? "Preserve every requested template section and do not add sections based on this mode."
    : "Follow the requested format and include only explicitly stated or unambiguous owners, commitments, and deadlines; do not turn proposals into commitments.";

  if (mode === "crisp") {
    return [
      "Summary mode: crisp. Make the summary fast to scan.",
      "Cover only decisions, outcomes, blockers, commitments, and the context required to understand them.",
      "Do not omit any explicit decision, blocker, owner, commitment, or deadline.",
      "Use short, direct sentences with one idea per sentence.",
      "Omit secondary discussion, repetition, conversational framing, minor examples, and rationale that did not affect the outcome without changing the requested structure.",
      templateGuidance,
    ].join(" ");
  }

  if (mode === "balanced") {
    return [
      "Summary mode: balanced. Keep the primary discussion complete while remaining concise.",
      "Do not omit any explicit decision, blocker, owner, commitment, or deadline.",
      "Include important supporting context and rationale, but omit repetition, tangents, and minor examples.",
      "Explain each key point briefly with enough context to understand it.",
      templateGuidance,
    ].join(" ");
  }

  return [
    "Summary mode: detailed. Capture every material topic, decision, rationale, example, open question, and commitment.",
    "Explain material points with concrete details and enough context to stand on their own.",
    "Retain useful secondary discussion and examples, but remove repetition and conversational filler.",
    templateGuidance,
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
    `Summary length mode "${policy.mode}" is ${getRelativeLengthDescription(policy.mode)}.`,
    hasTemplateSections
      ? `Keep every requested template section and stay under ${guidance.max_characters} characters overall.`
      : customFormat
        ? `Keep the requested structure and stay under ${guidance.max_characters} characters overall.`
        : `Keep the summary proportional to it: use ${sections} and stay under ${guidance.max_characters} characters overall.`,
    "A short meeting must produce a short summary; never pad with filler.",
  ].join(" ");
}
