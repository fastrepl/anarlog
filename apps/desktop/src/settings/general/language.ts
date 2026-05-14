const displayNames = new Intl.DisplayNames(["en"], { type: "language" });

export function getBaseLanguageDisplayName(code: string): string {
  const { language } = parseLocale(code);
  return displayNames.of(language) ?? code;
}

export function getBaseLanguageCode(code: string): string {
  return parseLocale(code).language;
}

export function parseLocale(code: string): {
  language: string;
  region?: string;
} {
  const locale = new Intl.Locale(code);
  return { language: locale.language, region: locale.region };
}
