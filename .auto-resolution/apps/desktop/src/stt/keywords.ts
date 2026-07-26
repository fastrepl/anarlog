export function normalizeKeywordList(words: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];

  for (const word of words) {
    const normalized = word.trim().replace(/\s+/g, " ");
    const key = normalized.toLocaleLowerCase();
    if (normalized.length < 2 || seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(normalized);
  }

  return result;
}

export function parseDictionaryTermsText(value: string): string[] {
  return normalizeKeywordList(
    value
      .split(/[\n,]/)
      .map((term) => term.trim())
      .filter(Boolean),
  );
}

export function formatDictionaryTerms(terms: string[]): string {
  return normalizeKeywordList(terms).join("\n");
}
