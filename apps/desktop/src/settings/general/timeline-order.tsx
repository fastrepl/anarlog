import { Trans, useLingui } from "@lingui/react/macro";

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
import { resolveTimelineOrder } from "~/sidebar/timeline/data";

export function TimelineOrderSelector() {
  const { t } = useLingui();
  const storedValue = useConfigValue("timeline_order");
  const setTimelineOrder = useSetSettingValue("timeline_order");
  const value = resolveTimelineOrder(storedValue);

  const options = [
    {
      value: "upcoming_first",
      label: t`Upcoming first`,
    },
    {
      value: "chronological",
      label: t`Oldest first`,
    },
  ] as const;

  return (
    <SettingRow
      title={<Trans>Sidebar timeline</Trans>}
      description={
        <Trans>
          Choose whether scrolling down moves toward older notes or toward
          upcoming meetings.
        </Trans>
      }
    >
      {(labelProps) => (
        <Select value={value} onValueChange={setTimelineOrder}>
          <SelectTrigger {...labelProps} className={SETTING_CONTROL_CLASS}>
            <SelectValue placeholder={t`Select timeline order`} />
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
