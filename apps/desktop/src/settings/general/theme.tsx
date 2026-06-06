import { Trans, useLingui } from "@lingui/react/macro";
import { useMemo } from "react";

import {
  SearchableSelect,
  type SearchableSelectOption,
} from "./searchable-select";

import { useConfigValue } from "~/shared/config";
import {
  applyDocumentTheme,
  writeStoredThemePreference,
} from "~/shared/theme/apply";
import type { ThemePreference } from "~/shared/theme/resolve";
import * as settings from "~/store/tinybase/store/settings";

const THEME_OPTIONS: ThemePreference[] = ["light", "dark", "system"];

export function ThemeSelector() {
  const { t } = useLingui();
  const value = useConfigValue("theme") as ThemePreference;
  const setTheme = settings.UI.useSetValueCallback(
    "theme",
    (next: string) => next,
    [],
    settings.STORE_ID,
  );

  const options: SearchableSelectOption[] = useMemo(
    () => [
      { value: "light", label: t`Light` },
      { value: "dark", label: t`Dark` },
      { value: "system", label: t`System` },
    ],
    [t],
  );

  return (
    <div className="flex flex-row items-center justify-between">
      <div>
        <h3 className="mb-1 text-sm font-medium">
          <Trans>Appearance</Trans>
        </h3>
        <p className="text-muted-foreground text-xs">
          <Trans>Choose light, dark, or match your system setting.</Trans>
        </p>
      </div>
      <SearchableSelect
        value={THEME_OPTIONS.includes(value) ? value : "system"}
        onChange={(next) => {
          const preference = next as ThemePreference;
          writeStoredThemePreference(preference);
          applyDocumentTheme(preference);
          setTheme(next);
        }}
        options={options}
        placeholder={t`Select appearance`}
        className="w-40"
      />
    </div>
  );
}
