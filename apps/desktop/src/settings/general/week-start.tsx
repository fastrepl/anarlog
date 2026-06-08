import { Trans, useLingui } from "@lingui/react/macro";
import { useMemo } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";

import { useConfigValue } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";

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
  const setWeekStart = settings.UI.useSetValueCallback(
    "week_start",
    (val: string) => val,
    [],
    settings.STORE_ID,
  );

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
    <div className="flex flex-row items-center justify-between">
      <div>
        <h3 className="mb-1 text-sm font-medium">
          <Trans>Week starts on</Trans>
        </h3>
        <p className="text-muted-foreground text-xs">
          <Trans>First day of the week in the calendar view</Trans>
        </p>
      </div>
      <Select value={displayValue} onValueChange={handleChange}>
        <SelectTrigger className="bg-card w-40 shadow-none focus:ring-0">
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
    </div>
  );
}
