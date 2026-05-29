import { setupI18n, type Messages } from "@lingui/core";

import { type DisplayLocale, SUPPORTED_DISPLAY_LOCALES } from "./locales";

const catalogModules = import.meta.glob<{ messages: Messages }>(
  "./locales/*/messages.ts",
  { eager: true },
);

const catalogs = SUPPORTED_DISPLAY_LOCALES.reduce(
  (acc, locale) => {
    const mod = catalogModules[`./locales/${locale}/messages.ts`];
    if (!mod) {
      throw new Error(`Missing i18n catalog for ${locale}`);
    }
    acc[locale] = mod.messages;
    return acc;
  },
  {} as Record<DisplayLocale, Messages>,
);

export function createI18n(locale: DisplayLocale) {
  const i18n = setupI18n();

  i18n.load(catalogs);
  i18n.activate(locale);

  return i18n;
}
