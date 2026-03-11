const displayNames = new Intl.DisplayNames(["en"], { type: "language" });

export function getLanguageDisplayName(code: string): string {
  return displayNames.of(code) ?? code;
}

export function getBaseLanguageDisplayName(code: string): string {
  const { language } = parseLocale(code);
  return displayNames.of(language) ?? code;
}

export function parseLocale(code: string): {
  language: string;
  region?: string;
} {
  const locale = new Intl.Locale(code);
  return { language: locale.language, region: locale.region };
}
