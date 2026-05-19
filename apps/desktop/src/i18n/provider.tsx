import { I18nProvider } from "@lingui/react";
import { type ReactNode, useMemo } from "react";

import { createI18n } from "./catalogs";
import { resolveDisplayLocale } from "./locales";

import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function AppI18nProvider({ children }: { children: ReactNode }) {
  const mainLanguage = useConfigValue("ai_language");
  const locale = resolveDisplayLocale(mainLanguage);
  const i18n = useMemo(() => createI18n(locale), [locale]);

  return (
    <I18nProvider i18n={i18n}>
      <DocumentLanguage key={locale} locale={locale} />
      {children}
    </I18nProvider>
  );
}

function DocumentLanguage({ locale }: { locale: string }) {
  useMountEffect(() => {
    document.documentElement.lang = locale;
  });

  return null;
}
