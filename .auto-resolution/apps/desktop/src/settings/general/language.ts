const displayNamesByLocale = new Map<string, Intl.DisplayNames>();

export const CORE_TRANSCRIPTION_LANGUAGE_CODES = [
  "ar",
  "be",
  "bg",
  "bn",
  "bs",
  "ca",
  "cs",
  "da",
  "de",
  "el",
  "en",
  "es",
  "et",
  "fa",
  "fi",
  "fr",
  "he",
  "hi",
  "hr",
  "hu",
  "id",
  "it",
  "ja",
  "kn",
  "ko",
  "lt",
  "lv",
  "mk",
  "mr",
  "ms",
  "nl",
  "no",
  "pl",
  "pt",
  "ro",
  "ru",
  "sk",
  "sl",
  "sr",
  "sv",
  "ta",
  "te",
  "th",
  "tl",
  "tr",
  "uk",
  "ur",
  "vi",
  "zh",
] as const;

export function getBaseLanguageDisplayName(
  code: string,
  displayLocale = "en",
): string {
  const language = getBaseLanguageCode(code);
  if (!language) {
    return code;
  }
  return getDisplayNames(displayLocale).of(language) ?? code;
}

export function getBaseLanguageCode(code: string): string {
  return tryParseLocale(code)?.language ?? "";
}

export function getAdditionalSpokenLanguages(
  mainLanguage: string | null | undefined,
  spokenLanguages: readonly string[] | null | undefined,
) {
  const mainLanguageCode = mainLanguage
    ? getBaseLanguageCode(mainLanguage)
    : null;
  const seen = new Set<string>();
  const languages: string[] = [];

  for (const spokenLanguage of spokenLanguages ?? []) {
    const code = getBaseLanguageCode(spokenLanguage);

    if (!code || code === mainLanguageCode || seen.has(code)) {
      continue;
    }

    seen.add(code);
    languages.push(code);
  }

  return languages;
}

export function parseLocale(code: string): {
  language: string;
  region?: string;
} {
  return tryParseLocale(code) ?? { language: "en" };
}

function tryParseLocale(code: string): {
  language: string;
  region?: string;
} | null {
  let locale: Intl.Locale;
  try {
    locale = new Intl.Locale(code);
  } catch {
    return null;
  }
  return { language: locale.language, region: locale.region };
}

function getDisplayNames(displayLocale: string) {
  const locale = getValidDisplayLocale(displayLocale);
  const existing = displayNamesByLocale.get(locale);

  if (existing) {
    return existing;
  }

  const displayNames = new Intl.DisplayNames([locale], { type: "language" });
  displayNamesByLocale.set(locale, displayNames);

  return displayNames;
}

function getValidDisplayLocale(displayLocale: string) {
  try {
    return new Intl.Locale(displayLocale).toString();
  } catch {
    return "en";
  }
}
