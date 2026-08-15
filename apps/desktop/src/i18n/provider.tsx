import { I18nProvider } from "@lingui/react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { type ReactNode } from "react";

import { createI18n } from "./catalogs";
import { resolveDisplayLocale } from "./locales";

import { useConfigValue } from "~/shared/config";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function AppI18nProvider({ children }: { children: ReactNode }) {
  const mainLanguage = useConfigValue("ai_language");
  const locale = resolveDisplayLocale(mainLanguage);
  const { data: i18n } = useQuery({
    queryKey: ["i18n-catalog", locale],
    queryFn: () => createI18n(locale),
    placeholderData: keepPreviousData,
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: Number.POSITIVE_INFINITY,
  });

  if (!i18n) return null;

  return (
    <I18nProvider i18n={i18n}>
      <DocumentLanguage key={i18n.locale} locale={i18n.locale} />
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
