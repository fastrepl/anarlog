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
import { settingControlStyles, SettingRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";

function getSystemWeekStart(): "sunday" | "monday" {
  const locale = navigator.language || "en-US";
  try {
    const options = new Intl.Locale(locale);
    const info = (options as any).getWeekInfo?.() ?? (options as any).weekInfo;
    if (info?.firstDay === 1) return "monday";
  } catch {}
  return "sunday";
}

export function WeekStartSelector() {
  const { t } = useLingui();
  const value = useConfigValue("week_start");
  const setWeekStart = useSetSettingValue("week_start");

  const systemDefault = useMemo(() => getSystemWeekStart(), []);

  const options = useMemo(
    () => [
      { value: "sunday", label: t`Sunday` },
      { value: "monday", label: t`Monday` },
    ],
    [t],
  );

  const displayValue = value || systemDefault;

  const handleChange = (val: string) => {
    setWeekStart(val === systemDefault ? "" : val);
  };

  return (
    <SettingRow
      title={<Trans>Week starts on</Trans>}
      description={<Trans>Choose which day begins your calendar week.</Trans>}
    >
      {(labelProps) => (
        <Select value={displayValue} onValueChange={handleChange}>
          <SelectTrigger {...labelProps} sx={settingControlStyles.control}>
            <SelectValue placeholder={t`Select day`} />
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
