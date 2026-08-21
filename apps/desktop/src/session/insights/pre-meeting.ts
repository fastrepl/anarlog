import { extractPlainText } from "~/search/contexts/engine/utils";
import type { PastSessionNote } from "~/session/insights/past-notes";

const MAX_FACTS = 3;
const MAX_SUMMARY_LENGTH = 320;
const SPACE_REGEX = /\s+/g;

export function getPreMeetingBriefFacts(note: PastSessionNote): string[] {
  const generatedFacts = note.summary
    ? note.summary
        .split("\n")
        .map((line) => compactBriefText(line, MAX_SUMMARY_LENGTH))
        .filter(Boolean)
        .slice(0, MAX_FACTS)
    : [];
  if (generatedFacts.length > 0) {
    return [...new Set(generatedFacts)];
  }

  const sourceSummary = compactBriefText(
    note.sourceSummary,
    MAX_SUMMARY_LENGTH,
  );
  return sourceSummary ? [sourceSummary] : [];
}

export function compactBriefText(value: string, maxLength: number): string {
  const text = extractPlainText(value)
    .replace(/<[^>]*>/g, " ")
    .replace(/!\[[^\]]*]\([^)]+\)/g, " ")
    .replace(/\[([^\]]+)]\([^)]+\)/g, "$1")
    .replace(/https?:\/\/\S+/gi, " ")
    .replace(/(^|\s)([-*+]|\d+[.)])\s+/g, " ")
    .replace(/[`_~>#]/g, "")
    .replace(SPACE_REGEX, " ")
    .trim();
  if (text.length <= maxLength) {
    return text;
  }

  const slice = text.slice(0, maxLength + 1);
  const lastSpace = slice.lastIndexOf(" ");
  const end = lastSpace > maxLength * 0.6 ? lastSpace : maxLength;
  return `${slice.slice(0, end).trim()}…`;
}
