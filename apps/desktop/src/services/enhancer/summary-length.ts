import type { Transcript } from "@hypr/plugin-template";

export const MIN_TRANSCRIPT_CHARACTERS_FOR_SUMMARY = 160;
export const SHORT_TRANSCRIPT_CHARACTER_LIMIT = 1_200;
export const MIN_SUMMARY_CHARACTERS = 320;
export const MAX_SUMMARY_GUIDANCE_CHARACTERS = 6_000;
const SECTION_GUIDANCE_CHARACTER_STEP = 2_000;
const MAX_GUIDANCE_SECTIONS = 8;

export type SummaryLengthPolicy = {
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

  return {
    transcriptCharacters,
    maxCharacters: Math.max(transcriptCharacters, MIN_SUMMARY_CHARACTERS),
    maxSections:
      transcriptCharacters < SHORT_TRANSCRIPT_CHARACTER_LIMIT ? 2 : null,
    guidance: {
      maxCharacters: clamp(
        transcriptCharacters,
        MIN_SUMMARY_CHARACTERS,
        MAX_SUMMARY_GUIDANCE_CHARACTERS,
      ),
      minSections: clamp(
        Math.ceil(transcriptCharacters / (SECTION_GUIDANCE_CHARACTER_STEP * 2)),
        1,
        5,
      ),
      maxSections: clamp(
        1 + Math.ceil(transcriptCharacters / SECTION_GUIDANCE_CHARACTER_STEP),
        2,
        MAX_GUIDANCE_SECTIONS,
      ),
    },
  };
}

export function formatSummaryLengthGuidance(
  policy: SummaryLengthPolicy | null,
): string | null {
  const guidance = policy?.guidance;
  if (!policy || !guidance) {
    return null;
  }

  const sections =
    guidance.minSections === guidance.maxSections
      ? `exactly ${guidance.maxSections} section${guidance.maxSections === 1 ? "" : "s"}`
      : `${guidance.minSections} to ${guidance.maxSections} sections`;

  return [
    `Summary length: the transcript contains about ${policy.transcriptCharacters} characters.`,
    `Keep the summary proportional to it: use ${sections} and stay under ${guidance.maxCharacters} characters overall.`,
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

    const truncatedLine = truncateLineToFit(
      keptLines,
      line,
      policy.maxCharacters,
    );
    if (truncatedLine) {
      keptLines.push(truncatedLine);
    }
    break;
  }

  return keptLines.join("\n").trim();
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

function truncateLineToFit(
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

  let truncated = characters.slice(0, low).join("").trimEnd();
  const lastSpace = truncated.lastIndexOf(" ");
  if (lastSpace >= Math.floor(truncated.length * 0.6)) {
    truncated = truncated.slice(0, lastSpace);
  }

  return truncated.replace(/[,:;\-–—]+$/u, "").trimEnd();
}
