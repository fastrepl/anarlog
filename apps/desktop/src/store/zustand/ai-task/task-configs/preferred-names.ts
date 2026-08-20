import { normalizeKeywordList } from "~/stt/keywords";

export function formatPreferredNamesGuidance(terms: string[]): string {
  const normalized = normalizeKeywordList(terms);
  if (normalized.length === 0) {
    return "";
  }

  return `# Preferred Names

Use these names and terms exactly when they appear, even if the transcript or notes spell them differently:
${normalized.map((term) => `- ${term}`).join("\n")}`;
}

export function appendPreferredNamesGuidance(
  prompt: string,
  terms: string[],
): string {
  const guidance = formatPreferredNamesGuidance(terms);
  return guidance ? `${prompt}\n\n${guidance}` : prompt;
}
