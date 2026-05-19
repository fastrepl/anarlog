const displayNamesByLocale = new Map<string, Intl.DisplayNames>();

export function getBaseLanguageDisplayName(
  code: string,
  displayLocale = "en",
): string {
  const { language } = parseLocale(code);
  return getDisplayNames(displayLocale).of(language) ?? code;
}

export function getBaseLanguageCode(code: string): string {
  return parseLocale(code).language;
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
  const locale = new Intl.Locale(code);
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
