import { useMemo } from "react";

import {
  SearchableSelect,
  type SearchableSelectOption,
} from "./searchable-select";

import { useConfigValue } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";

const COMMON_TIMEZONES = [
  { value: "Pacific/Honolulu", label: "Hawaii", detail: "UTC-10" },
  { value: "America/Anchorage", label: "Alaska", detail: "UTC-9" },
  { value: "America/Los_Angeles", label: "Pacific Time", detail: "UTC-8" },
  { value: "America/Denver", label: "Mountain Time", detail: "UTC-7" },
  { value: "America/Chicago", label: "Central Time", detail: "UTC-6" },
  { value: "America/New_York", label: "Eastern Time", detail: "UTC-5" },
  { value: "America/Sao_Paulo", label: "Sao Paulo", detail: "UTC-3" },
  { value: "Atlantic/Reykjavik", label: "Reykjavik", detail: "UTC+0" },
  { value: "Europe/London", label: "London", detail: "UTC+0/+1" },
  { value: "Europe/Paris", label: "Paris", detail: "UTC+1/+2" },
  { value: "Europe/Berlin", label: "Berlin", detail: "UTC+1/+2" },
  { value: "Africa/Cairo", label: "Cairo", detail: "UTC+2" },
  { value: "Europe/Moscow", label: "Moscow", detail: "UTC+3" },
  { value: "Asia/Dubai", label: "Dubai", detail: "UTC+4" },
  { value: "Asia/Kolkata", label: "India", detail: "UTC+5:30" },
  { value: "Asia/Bangkok", label: "Bangkok", detail: "UTC+7" },
  { value: "Asia/Singapore", label: "Singapore", detail: "UTC+8" },
  { value: "Asia/Shanghai", label: "China", detail: "UTC+8" },
  { value: "Asia/Tokyo", label: "Tokyo", detail: "UTC+9" },
  { value: "Asia/Seoul", label: "Seoul", detail: "UTC+9" },
  { value: "Australia/Sydney", label: "Sydney", detail: "UTC+10/+11" },
  { value: "Pacific/Auckland", label: "Auckland", detail: "UTC+12/+13" },
];

export function TimezoneSelector() {
  const value = useConfigValue("timezone");
  const setTimezone = settings.UI.useSetValueCallback(
    "timezone",
    (val: string) => val,
    [],
    settings.STORE_ID,
  );

  const systemTimezone = useMemo(() => {
    return Intl.DateTimeFormat().resolvedOptions().timeZone;
  }, []);

  const options: SearchableSelectOption[] = useMemo(() => COMMON_TIMEZONES, []);

  const displayValue = value || systemTimezone;

  const handleChange = (val: string) => {
    setTimezone(val === systemTimezone ? "" : val);
  };

  return (
    <div className="flex flex-row items-center justify-between">
      <div>
        <h3 className="mb-1 text-sm font-medium">Timezone</h3>
        <p className="text-xs text-neutral-600">
          Override the timezone used for the sidebar timeline
        </p>
      </div>
      <SearchableSelect
        value={displayValue}
        onChange={handleChange}
        options={options}
        placeholder="Select timezone"
        searchPlaceholder="Search timezone..."
        className="w-40"
      />
    </div>
  );
}
