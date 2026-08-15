import { setupI18n, type I18n, type Messages } from "@lingui/core";

import { SOURCE_LOCALE, type DisplayLocale } from "./locales";

const catalogModules = import.meta.glob<{ messages: Messages }>(
  "./locales/*/messages.ts",
);
const catalogCache = new Map<DisplayLocale, Promise<Messages>>();

export function getCatalogLocalesForDisplayLocale(
  locale: DisplayLocale,
): DisplayLocale[] {
  return locale === SOURCE_LOCALE ? [SOURCE_LOCALE] : [SOURCE_LOCALE, locale];
}

export async function createI18n(locale: DisplayLocale): Promise<I18n> {
  const i18n = setupI18n();
  const locales = getCatalogLocalesForDisplayLocale(locale);
  const messages = await Promise.all(locales.map(loadCatalog));
  const sourceMessages = messages[0]!;
  const activeMessages = messages[messages.length - 1]!;

  i18n.load(SOURCE_LOCALE, sourceMessages);
  i18n.load(locale, { ...sourceMessages, ...activeMessages });
  i18n.activate(locale);

  return i18n;
}

function loadCatalog(locale: DisplayLocale): Promise<Messages> {
  const cached = catalogCache.get(locale);
  if (cached) return cached;

  const path = `./locales/${locale}/messages.ts`;
  const load = catalogModules[path];
  if (!load) {
    return Promise.reject(new Error(`Missing i18n catalog: ${locale}`));
  }

  const pending = load()
    .then((module) => module.messages)
    .catch((error: unknown) => {
      catalogCache.delete(locale);
      throw error;
    });
  catalogCache.set(locale, pending);
  return pending;
}
