const displayNames = new Intl.DisplayNames(["en"], { type: "language" });

export function getBaseLanguageDisplayName(code: string): string {
  const { language } = parseLocale(code);
  return displayNames.of(language) ?? code;
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
