import { setupI18n, type Messages } from "@lingui/core";

import { SOURCE_LOCALE, type DisplayLocale } from "./locales";

const catalogModules = import.meta.glob<{ messages: Messages }>(
  "./locales/*/messages.ts",
);

const catalogLoaders = Object.fromEntries(
  Object.entries(catalogModules).map(([path, module]) => {
    const locale = path.match(/^\.\/locales\/([^/]+)\/messages\.ts$/)?.[1];

    if (!locale) {
      throw new Error(`Invalid i18n catalog path: ${path}`);
    }

    return [locale, module];
  }),
) as Record<DisplayLocale, () => Promise<{ messages: Messages }>>;

export async function createI18n(locale: DisplayLocale) {
  const i18n = setupI18n();
  const loadCatalog = catalogLoaders[locale] ?? catalogLoaders[SOURCE_LOCALE];

  if (!loadCatalog) {
    throw new Error(`Missing i18n catalog for locale: ${locale}`);
  }

  const { messages } = await loadCatalog();
  i18n.load(locale, messages);
  i18n.activate(locale);

  return i18n;
}
