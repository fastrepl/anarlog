import { Trans, useLingui } from "@lingui/react/macro";
import { Settings2Icon } from "lucide-react";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";

import { LegacyMigrationCleanupRow } from "./legacy-cleanup";

import { useSetSettingValues } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

const AUDIO_RETENTION_OPTIONS = [
  {
    value: "none",
    label: "Don't save",
  },
  {
    value: "oneDay",
    label: "1 day",
  },
  {
    value: "threeDays",
    label: "3 days",
  },
  {
    value: "oneWeek",
    label: "1 week",
  },
  {
    value: "oneMonth",
    label: "1 month",
  },
  {
    value: "forever",
    label: "Forever",
  },
];

export function StorageSettingsView() {
  return (
    <div>
      <h2 className="mb-4 font-sans text-lg font-semibold">
        <Trans>Storage</Trans>
      </h2>
      <div className="flex flex-col gap-3">
        <AudioRetentionRow />
        <LegacyMigrationCleanupRow />
      </div>
    </div>
  );
}

function AudioRetentionRow() {
  const { t } = useLingui();
  const audioRetention = useConfigValue("audio_retention") || "forever";
  const setSettingValues = useSetSettingValues();
  const setAudioRetention = (value: string) => {
    setSettingValues({
      audio_retention: value,
      save_recordings: value !== "none",
    });
  };
  const copyByValue = {
    none: t`Don't save`,
    oneDay: t`1 day`,
    threeDays: t`3 days`,
    oneWeek: t`1 week`,
    oneMonth: t`1 month`,
    forever: t`Forever`,
  } as const;

  return (
    <div className="grid grid-cols-[minmax(0,1fr)_9rem] items-center gap-3">
      <div className="flex min-w-0 cursor-default items-center gap-2">
        <Settings2Icon className="text-muted-foreground size-4" />
        <span className="truncate text-sm font-medium">
          <Trans>Audio file retention</Trans>
        </span>
      </div>
      <Select value={audioRetention} onValueChange={setAudioRetention}>
        <SelectTrigger className="bg-card h-9 w-full shadow-none focus:ring-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {AUDIO_RETENTION_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {copyByValue[option.value as keyof typeof copyByValue] ??
                option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
