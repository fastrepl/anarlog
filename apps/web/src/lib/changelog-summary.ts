const FALLBACK_SUMMARY = "See what changed in this release.";

export function getEntrySummary(content: string) {
  const summary = content.split("\n").map(cleanSummaryLine).find(Boolean);

  if (!summary) {
    return FALLBACK_SUMMARY;
  }

  return firstSentence(summary);
}

function cleanSummaryLine(line: string) {
  const trimmed = line.trim();

  if (
    !trimmed ||
    trimmed.startsWith("#") ||
    trimmed.startsWith("![") ||
    /^<\/?[a-z][^>]*>$/i.test(trimmed)
  ) {
    return "";
  }

  return trimmed
    .replace(/^[-*]\s+/, "")
    .replace(/^\d+\.\s+/, "")
    .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/\*([^*]+)\*/g, "$1")
    .replace(/__([^_]+)__/g, "$1")
    .replace(/_([^_]+)_/g, "$1")
    .replace(/<\/?[a-z][^>]*>/gi, "")
    .replace(/\s+/g, " ")
    .trim();
}

function firstSentence(value: string) {
  return value.match(/^.*?[.!?](?=\s|$)/)?.[0] ?? value;
}
