import { Trans, useLingui } from "@lingui/react/macro";
import type { ReactNode } from "react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

import {
  normalizeSummaryLengthMode,
  type SummaryLengthMode,
} from "~/services/enhancer/summary-length";
import { useSetSettingValue } from "~/settings/queries";
import { settingControlStyles, SettingRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";

export function SummaryLengthSelector() {
  const { t } = useLingui();
  const value = normalizeSummaryLengthMode(useConfigValue("summary_length"));
  const setSummaryLength = useSetSettingValue("summary_length");

  const handleChange = (nextValue: string) => {
    setSummaryLength(normalizeSummaryLengthMode(nextValue));
  };

  return (
    <SettingRow
      title={<Trans>Summary length</Trans>}
      description={
        <Trans>
          Choose how much detail generated meeting summaries include.
        </Trans>
      }
    >
      {(labelProps) => (
        <Select value={value} onValueChange={handleChange}>
          <SelectTrigger {...labelProps} sx={settingControlStyles.control}>
            <SelectValue placeholder={t`Select summary length`} />
          </SelectTrigger>
          <SelectContent>
            <SummaryLengthOption value="crisp">
              <Trans>Crisp</Trans>
            </SummaryLengthOption>
            <SummaryLengthOption value="balanced">
              <Trans>Balanced</Trans>
            </SummaryLengthOption>
            <SummaryLengthOption value="detailed">
              <Trans>Detailed</Trans>
            </SummaryLengthOption>
          </SelectContent>
        </Select>
      )}
    </SettingRow>
  );
}

function SummaryLengthOption({
  value,
  children,
}: {
  value: SummaryLengthMode;
  children: ReactNode;
}) {
  return <SelectItem value={value}>{children}</SelectItem>;
}
