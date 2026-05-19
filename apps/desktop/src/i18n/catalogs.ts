import { setupI18n, type Messages } from "@lingui/core";

import type { DisplayLocale } from "./locales";
import { messages as enMessages } from "./locales/en/messages";
import { messages as jaMessages } from "./locales/ja/messages";
import { messages as koMessages } from "./locales/ko/messages";

const catalogs: Record<DisplayLocale, Messages> = {
  en: enMessages,
  ko: koMessages,
  ja: jaMessages,
};

export function createI18n(locale: DisplayLocale) {
  const i18n = setupI18n();

  i18n.load(catalogs);
  i18n.activate(locale);

  return i18n;
}
