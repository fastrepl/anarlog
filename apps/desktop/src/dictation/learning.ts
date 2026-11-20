const MAX_SUGGESTIONS = 20;
const MAX_SEEN = 200;

export type DictionarySuggestions = {
  accountId: string;
  terms: string[];
  dismissed: string[];
};

/** Suggestions belong to one account; another account's blob reads as empty. */
export function parseSuggestions(
  value: string | undefined,
  accountId: string | null,
): DictionarySuggestions | null {
  if (!accountId) return null;
  const empty = { accountId, terms: [], dismissed: [] };
  if (!value) return empty;
  try {
    const parsed = JSON.parse(value) as Partial<DictionarySuggestions>;
    if (parsed.accountId !== accountId) return empty;
    return {
      accountId,
      terms: strings(parsed.terms).slice(0, MAX_SUGGESTIONS),
      dismissed: strings(parsed.dismissed).slice(-MAX_SEEN),
    };
  } catch {
    return empty;
  }
}

export function addSuggestions(
  current: DictionarySuggestions,
  terms: readonly string[],
): DictionarySuggestions {
  const known = new Set(
    [...current.terms, ...current.dismissed].map((term) => term.toLowerCase()),
  );
  const added = terms.filter((term) => !known.has(term.toLowerCase()));
  return {
    ...current,
    terms: [...current.terms, ...added].slice(-MAX_SUGGESTIONS),
  };
}

export function settleSuggestion(
  current: DictionarySuggestions,
  term: string,
): DictionarySuggestions {
  return {
    ...current,
    terms: current.terms.filter((value) => value !== term),
    dismissed: [...current.dismissed, term].slice(-MAX_SEEN),
  };
}

export type Replacement = { from: string; to: string };

export function parseReplacements(value: string | undefined): Replacement[] {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (entry): entry is Replacement =>
        !!entry &&
        typeof entry === "object" &&
        typeof (entry as Replacement).from === "string" &&
        typeof (entry as Replacement).to === "string" &&
        (entry as Replacement).from.trim() !== "",
    );
  } catch {
    return [];
  }
}

/**
 * What remains of an inserted dictation once the person edited it: the text between what
 * surrounded the insertion, or null once that surrounding text changed too.
 */
export function keptBetween(before: string, after: string, now: string) {
  if (!now.startsWith(before) || !now.endsWith(after)) return null;
  if (now.length < before.length + after.length) return null;
  return now.slice(before.length, now.length - after.length).trim() || null;
}

function strings(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === "string")
    : [];
}
