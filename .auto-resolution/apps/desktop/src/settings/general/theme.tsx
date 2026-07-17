import { Trans, useLingui } from "@lingui/react/macro";
import { useMemo } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";

import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import {
  applyDocumentTheme,
  writeStoredThemePreference,
} from "~/shared/theme/apply";
import type { ThemePreference } from "~/shared/theme/resolve";

const THEME_OPTIONS: ThemePreference[] = ["light", "dark", "system"];

export function ThemeSelector() {
  const { t } = useLingui();
  const value = useConfigValue("theme") as ThemePreference;
  const setTheme = useSetSettingValue("theme");

  const options = useMemo(
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
      <Select
        value={THEME_OPTIONS.includes(value) ? value : "system"}
        onValueChange={(next) => {
          const preference = next as ThemePreference;
          writeStoredThemePreference(preference);
          applyDocumentTheme(preference);
          setTheme(next);
        }}
      >
        <SelectTrigger className="bg-card w-40 shadow-none focus:ring-0">
          <SelectValue placeholder={t`Select appearance`} />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
