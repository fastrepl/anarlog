import type { Transcript } from "@anlg/plugin-template";

export const MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY = 160;
const SHORT_TRANSCRIPT_CHARACTER_LIMIT = 1_200;
export const MIN_SUMMARY_CHARACTERS = 320;
const SECTION_GUIDANCE_CHARACTER_STEP = 2_000;
const TEMPLATE_SECTION_MIN_CHARACTERS = 150;
const MAX_GUIDANCE_SECTIONS = 8;

const SUMMARY_LENGTH_MODES = ["crisp", "balanced", "detailed"] as const;
export type SummaryLengthMode = (typeof SUMMARY_LENGTH_MODES)[number];
const DEFAULT_SUMMARY_LENGTH_MODE: SummaryLengthMode = "detailed";

const SUMMARY_LENGTH_RATIOS: Record<SummaryLengthMode, number> = {
  crisp: 0.25,
  balanced: 0.5,
  detailed: 1,
};

export type SummaryLengthPolicy = {
  mode: SummaryLengthMode;
  maxCharacters: number;
  maxSections: number | null;
  transcriptCharacters: number;
  guidance?: {
    maxCharacters: number;
    minSections: number;
    maxSections: number;
  };
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

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

export function getSummaryLengthPolicy(
  transcripts: readonly Transcript[],
  mode: SummaryLengthMode = DEFAULT_SUMMARY_LENGTH_MODE,
  customFormat = false,
  templateSectionCount = 0,
): SummaryLengthPolicy | null {
  const transcriptCharacters = countNormalizedCharacters(
    transcripts
      .flatMap((transcript) => transcript.segments)
      .map((segment) => segment.text)
      .filter(Boolean)
      .join(" "),
  );

  if (transcriptCharacters === 0) {
    return null;
  }

  const ratio = SUMMARY_LENGTH_RATIOS[mode];
  const baseMinSections = clamp(
    Math.ceil(transcriptCharacters / (SECTION_GUIDANCE_CHARACTER_STEP * 2)),
    1,
    5,
  );
  const baseMaxSections = clamp(
    1 + Math.ceil(transcriptCharacters / SECTION_GUIDANCE_CHARACTER_STEP),
    2,
    MAX_GUIDANCE_SECTIONS,
  );

  return {
    mode,
    transcriptCharacters,
    maxCharacters: Math.max(
      Math.round(Math.max(transcriptCharacters, MIN_SUMMARY_CHARACTERS)),
      MIN_SUMMARY_CHARACTERS,
    ),
    maxSections:
      !customFormat && transcriptCharacters < SHORT_TRANSCRIPT_CHARACTER_LIMIT
        ? 2
        : null,
    guidance: {
      maxCharacters: Math.max(
        Math.max(
          Math.round(transcriptCharacters * ratio),
          MIN_SUMMARY_CHARACTERS,
        ),
        templateSectionCount * TEMPLATE_SECTION_MIN_CHARACTERS,
      ),
      minSections: Math.ceil(baseMinSections * ratio),
      maxSections: Math.max(2, Math.ceil(baseMaxSections * ratio)),
    },
  };
}

export function normalizeSummaryLengthMode(value: unknown): SummaryLengthMode {
  return SUMMARY_LENGTH_MODES.includes(value as SummaryLengthMode)
    ? (value as SummaryLengthMode)
    : DEFAULT_SUMMARY_LENGTH_MODE;
}

export function getRelativeLengthDescription(mode: SummaryLengthMode): string {
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
    guidance.minSections === guidance.maxSections
      ? `exactly ${guidance.maxSections} section${guidance.maxSections === 1 ? "" : "s"}`
      : `${guidance.minSections} to ${guidance.maxSections} sections`;

  return [
    `Summary length: the transcript contains about ${policy.transcriptCharacters} characters.`,
    `Summary length mode "${policy.mode}" is ${getRelativeLengthDescription(policy.mode)}.`,
    hasTemplateSections
      ? `Keep every requested template section and stay under ${guidance.maxCharacters} characters overall.`
      : customFormat
        ? `Keep the requested structure and stay under ${guidance.maxCharacters} characters overall.`
        : `Keep the summary proportional to it: use ${sections} and stay under ${guidance.maxCharacters} characters overall.`,
    "A short meeting must produce a short summary; never pad with filler.",
  ].join(" ");
}

export function constrainSummaryLength(
  markdown: string,
  policy: SummaryLengthPolicy | null,
): string {
  if (!policy) {
    return markdown.trim();
  }

  const sectionLimited = limitSections(markdown, policy.maxSections);
  if (countNormalizedCharacters(sectionLimited) <= policy.maxCharacters) {
    return sectionLimited;
  }

  const keptLines: string[] = [];
  for (const line of sectionLimited.split("\n")) {
    const candidate = [...keptLines, line].join("\n").trim();
    if (countNormalizedCharacters(candidate) <= policy.maxCharacters) {
      keptLines.push(line);
      continue;
    }

    const truncatedLine = truncateLineToSafeBoundary(
      keptLines,
      line,
      policy.maxCharacters,
    );
    if (truncatedLine) {
      keptLines.push(truncatedLine);
    }
    break;
  }

  return removeTrailingEmptyHeading(keptLines).join("\n").trim();
}

function limitSections(markdown: string, maxSections: number | null): string {
  if (!maxSections) {
    return markdown.trim();
  }

  let sectionCount = 0;
  const keptLines: string[] = [];
  for (const line of markdown.trim().split("\n")) {
    if (/^#\s+\S/.test(line)) {
      sectionCount += 1;
      if (sectionCount > maxSections) {
        break;
      }
    }
    keptLines.push(line);
  }

  return keptLines.join("\n").trim();
}

function truncateLineToSafeBoundary(
  keptLines: string[],
  line: string,
  maxCharacters: number,
): string {
  const characters = Array.from(line);
  let low = 0;
  let high = characters.length;

  while (low < high) {
    const midpoint = Math.ceil((low + high) / 2);
    const candidate = [...keptLines, characters.slice(0, midpoint).join("")]
      .join("\n")
      .trim();
    if (countNormalizedCharacters(candidate) <= maxCharacters) {
      low = midpoint;
    } else {
      high = midpoint - 1;
    }
  }

  const truncated = characters.slice(0, low).join("").trimEnd();
  const sentenceEndings = [...truncated.matchAll(/[.!?](?=\s|$)/gu)];
  const lastSentenceEnding = sentenceEndings[sentenceEndings.length - 1];
  if (!lastSentenceEnding?.index) {
    return truncated.match(/^(.+\S)\s+\S*$/u)?.[1] ?? truncated;
  }

  return truncated.slice(
    0,
    lastSentenceEnding.index + lastSentenceEnding[0].length,
  );
}

function removeTrailingEmptyHeading(lines: string[]): string[] {
  let lastContentIndex = lines.length - 1;
  while (lastContentIndex >= 0 && !lines[lastContentIndex].trim()) {
    lastContentIndex -= 1;
  }

  if (/^#{1,6}\s+\S/u.test(lines[lastContentIndex] ?? "")) {
    return lines.slice(0, lastContentIndex);
  }

  return lines;
}
