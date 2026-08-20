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

export function parseDictionaryTermsJson(value: unknown): string[] {
  if (Array.isArray(value)) {
    return normalizeKeywordList(
      value.filter((term): term is string => typeof term === "string"),
    );
  }

  if (typeof value !== "string") {
    return [];
  }

  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? normalizeKeywordList(
          parsed.filter((term): term is string => typeof term === "string"),
        )
      : [];
  } catch {
    return [];
  }
}
