import { Trans, useLingui } from "@lingui/react/macro";
import { useMemo } from "react";

import { getBaseLanguageDisplayName, parseLocale } from "./language";
import {
  SearchableSelect,
  type SearchableSelectOption,
} from "./searchable-select";

export function MainLanguageView({
  value,
  onChange,
  supportedLanguages,
}: {
  value: string;
  onChange: (value: string) => void;
  supportedLanguages: readonly string[];
}) {
  const { i18n, t } = useLingui();

  const deduped = useMemo(() => {
    const map = new Map<string, string>();
    for (const code of supportedLanguages) {
      const { language } = parseLocale(code);
      if (!map.has(language)) {
        map.set(language, code);
      }
    }
    return map;
  }, [supportedLanguages]);

  const normalizedValue = useMemo(() => {
    const { language } = parseLocale(value);
    return deduped.get(language) ?? value;
  }, [value, deduped]);

  const options: SearchableSelectOption[] = useMemo(
    () =>
      [...deduped.values()].map((code) => ({
        value: code,
        label: getBaseLanguageDisplayName(code, i18n.locale),
      })),
    [deduped, i18n.locale],
  );

  return (
    <div className="flex flex-row items-center justify-between">
      <div>
        <h3 className="mb-1 text-sm font-medium">
          <Trans>Main language</Trans>
        </h3>
        <p className="text-xs text-neutral-600">
          <Trans>
            Language for summaries, chats, and AI-generated responses
          </Trans>
        </p>
      </div>
      <SearchableSelect
        value={normalizedValue}
        onChange={onChange}
        options={options}
        placeholder={t`Select language`}
        searchPlaceholder={t`Search language...`}
        emptyMessage={t`No matching languages found`}
        className="w-48"
      />
    </div>
  );
}
