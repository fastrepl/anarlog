export const SOURCE_LOCALE = "en";

export const SUPPORTED_DISPLAY_LOCALES = [
  "af",
  "am",
  "ar",
  "as",
  "az",
  "ba",
  "be",
  "bg",
  "bn",
  "bo",
  "br",
  "bs",
  "ca",
  "cs",
  "cy",
  "da",
  "de",
  "el",
  "en",
  "es",
  "et",
  "eu",
  "fa",
  "ff",
  "fi",
  "fo",
  "fr",
  "ga",
  "gl",
  "gu",
  "ha",
  "he",
  "hi",
  "hr",
  "ht",
  "hu",
  "hy",
  "id",
  "ig",
  "is",
  "it",
  "ja",
  "jv",
  "ka",
  "kk",
  "km",
  "kn",
  "ko",
  "ku",
  "ky",
  "la",
  "lb",
  "lg",
  "ln",
  "lo",
  "lt",
  "lv",
  "mg",
  "mi",
  "mk",
  "ml",
  "mn",
  "mr",
  "ms",
  "mt",
  "my",
  "ne",
  "nl",
  "nn",
  "no",
  "ny",
  "oc",
  "or",
  "pa",
  "pl",
  "ps",
  "pt",
  "ro",
  "ru",
  "sa",
  "sd",
  "si",
  "sk",
  "sl",
  "sn",
  "so",
  "sq",
  "sr",
  "su",
  "sv",
  "sw",
  "ta",
  "te",
  "tg",
  "th",
  "tk",
  "tl",
  "tr",
  "tt",
  "uk",
  "ur",
  "uz",
  "vi",
  "wo",
  "xh",
  "yi",
  "yo",
  "zh",
  "zu",
] as const;

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
