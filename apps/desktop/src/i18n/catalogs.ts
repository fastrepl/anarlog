import { setupI18n, type Messages } from "@lingui/core";

import type { DisplayLocale } from "./locales";
import { messages as deMessages } from "./locales/de/messages";
import { messages as enMessages } from "./locales/en/messages";
import { messages as esMessages } from "./locales/es/messages";
import { messages as frMessages } from "./locales/fr/messages";
import { messages as itMessages } from "./locales/it/messages";
import { messages as jaMessages } from "./locales/ja/messages";
import { messages as koMessages } from "./locales/ko/messages";
import { messages as ptMessages } from "./locales/pt/messages";
import { messages as zhMessages } from "./locales/zh/messages";

const catalogs: Record<DisplayLocale, Messages> = {
  en: enMessages,
  de: deMessages,
  es: esMessages,
  fr: frMessages,
  it: itMessages,
  ja: jaMessages,
  ko: koMessages,
  pt: ptMessages,
  zh: zhMessages,
};

export function createI18n(locale: DisplayLocale) {
  const i18n = setupI18n();

  i18n.load(catalogs);
  i18n.activate(locale);

  return i18n;
}
