import { setupI18n, type Messages } from "@lingui/core";

import type { DisplayLocale } from "./locales";

const catalogModules = import.meta.glob<{ messages: Messages }>(
  "./locales/*/messages.ts",
  { eager: true },
);

const catalogs = Object.fromEntries(
  Object.entries(catalogModules).map(([path, module]) => {
    const locale = path.match(/^\.\/locales\/([^/]+)\/messages\.ts$/)?.[1];

    if (!locale) {
      throw new Error(`Invalid i18n catalog path: ${path}`);
    }

    return [locale, module.messages];
  }),
) as Record<DisplayLocale, Messages>;

export function createI18n(locale: DisplayLocale) {
  const i18n = setupI18n();

  i18n.load(catalogs);
  i18n.activate(locale);

  return i18n;
}
