import { Trans, useLingui } from "@lingui/react/macro";
import { useMemo } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

import { useSetSettingValue } from "~/settings/queries";
import { SETTING_CONTROL_CLASS, SettingRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";
import { applyThemePreference } from "~/shared/theme/provider";
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
    <SettingRow
      title={<Trans>Appearance</Trans>}
      description={
        <Trans>Choose light, dark, or match your system setting.</Trans>
      }
    >
      {(labelProps) => (
        <Select
          value={THEME_OPTIONS.includes(value) ? value : "system"}
          onValueChange={(next) => {
            const preference = next as ThemePreference;
            void applyThemePreference(preference);
            setTheme(next);
          }}
        >
          <SelectTrigger {...labelProps} className={SETTING_CONTROL_CLASS}>
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
      )}
    </SettingRow>
  );
}
