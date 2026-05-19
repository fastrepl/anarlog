export const SOURCE_LOCALE = "en";

export const SUPPORTED_DISPLAY_LOCALES = ["en", "ko", "ja"] as const;

export type DisplayLocale = (typeof SUPPORTED_DISPLAY_LOCALES)[number];

const supportedDisplayLocales = new Set<string>(SUPPORTED_DISPLAY_LOCALES);

export function resolveDisplayLocale(
  language: string | null | undefined,
): DisplayLocale {
  if (!language) {
    return SOURCE_LOCALE;
  }

  let locale: Intl.Locale;
  try {
    locale = new Intl.Locale(language);
  } catch {
    return SOURCE_LOCALE;
  }

  const exactLocale = locale.toString();
  if (supportedDisplayLocales.has(exactLocale)) {
    return exactLocale as DisplayLocale;
  }

  if (supportedDisplayLocales.has(locale.language)) {
    return locale.language as DisplayLocale;
  }

  return SOURCE_LOCALE;
}
