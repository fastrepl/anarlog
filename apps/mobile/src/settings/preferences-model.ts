export const SHARED_PREFERENCE_KEYS = [
  "theme",
  "sidebar_show_folder",
  "sidebar_show_tags",
] as const;

export const DEFAULT_PREFERENCES = {
  ai_language: "en",
  spoken_languages: [] as string[],
  personalization_dictionary_terms: [] as string[],
  summary_length: "detailed" as "crisp" | "balanced" | "detailed",
  theme: "system" as "system" | "light" | "dark",
  sidebar_show_folder: true,
  sidebar_show_tags: false,
};

export type Preferences = typeof DEFAULT_PREFERENCES;
export type PreferenceKey = keyof Preferences;
export const PREFERENCE_KEYS = Object.keys(
  DEFAULT_PREFERENCES,
) as PreferenceKey[];
export type PreferenceRow = {
  id: string;
  value_json: string;
  source_rank: number;
};

export function normalizeDictionary(terms: readonly string[]): string[] {
  const seen = new Set<string>();
  return terms
    .map((term) => term.trim().replace(/\s+/g, " "))
    .filter((term) => {
      const normalized = term.toLocaleLowerCase();
      if (!term || term.length > 100 || seen.has(normalized)) return false;
      seen.add(normalized);
      return true;
    })
    .slice(0, 100);
}

export function parsePreferences(rows: PreferenceRow[]): Preferences {
  const result = { ...DEFAULT_PREFERENCES };
  for (const row of [...rows].sort((a, b) => a.source_rank - b.source_rank)) {
    let value: unknown;
    try {
      value = JSON.parse(row.value_json);
    } catch {
      continue;
    }
    switch (row.id) {
      case "sidebar_show_folder":
      case "sidebar_show_tags":
        if (typeof value === "boolean") result[row.id] = value;
        break;
      case "theme":
        if (value === "system" || value === "light" || value === "dark")
          result.theme = value;
        break;
      case "summary_length":
        if (value === "crisp" || value === "balanced" || value === "detailed")
          result.summary_length = value;
        break;
      case "ai_language":
        if (
          typeof value === "string" &&
          /^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(value)
        )
          result.ai_language = value;
        break;
      case "spoken_languages":
      case "personalization_dictionary_terms": {
        // Desktop stores list preferences as JSON strings inside value_json.
        if (typeof value === "string") {
          try {
            value = JSON.parse(value);
          } catch {
            continue;
          }
        }
        if (
          !Array.isArray(value) ||
          !value.every((item) => typeof item === "string")
        )
          continue;
        result[row.id] =
          row.id === "spoken_languages"
            ? [
                ...new Set(
                  value.filter((code) =>
                    /^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(code),
                  ),
                ),
              ].slice(0, 8)
            : normalizeDictionary(value);
        break;
      }
    }
  }
  return result;
}

export function serializePreference<K extends PreferenceKey>(
  key: K,
  value: Preferences[K],
): string {
  return JSON.stringify(
    key === "spoken_languages" || key === "personalization_dictionary_terms"
      ? JSON.stringify(value)
      : value,
  );
}

export function applyTranscriptionPreferences(
  url: URL,
  preferences: Preferences,
): URL {
  for (const language of new Set([
    preferences.ai_language,
    ...preferences.spoken_languages,
  ])) {
    url.searchParams.append("language", language);
  }
  for (const term of preferences.personalization_dictionary_terms) {
    url.searchParams.append("keywords", term);
  }
  return url;
}
