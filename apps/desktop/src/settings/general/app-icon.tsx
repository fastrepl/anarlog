import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { platform } from "@tauri-apps/plugin-os";

import { cn } from "@anlg/utils";

import { useSetSettingValue } from "~/settings/queries";
import { SettingRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";
import {
  type AppIconPreference,
  normalizeAppIconPreference,
  resolveAppIconName,
} from "~/shared/theme/icon";
import { applyAppIconPreference } from "~/shared/theme/provider";

const APP_ICON_OPTIONS = [
  "default",
  "stable",
  "anagram",
  "dev",
  "staging",
] as const satisfies readonly AppIconPreference[];

export function AppIconSelector() {
  const { t } = useLingui();
  const value = normalizeAppIconPreference(useConfigValue("app_icon"));
  const setAppIcon = useSetSettingValue("app_icon");
  const { data: appIdentifier = "com.hyprnote.dev" } = useQuery({
    queryKey: ["tauri", "app-identifier"],
    queryFn: getIdentifier,
    staleTime: Infinity,
  });
  const labels = {
    default: t`Default`,
    stable: t`Production`,
    anagram: t`Anagram`,
    dev: t`Blueprint`,
    staging: t`Sketch`,
  };

  if (platform() !== "macos") {
    return null;
  }

  return (
    <SettingRow
      title={<Trans>App icon</Trans>}
      description={
        <Trans>Choose the icon shown in the Dock and app switcher.</Trans>
      }
    >
      {(labelProps) => (
        <div
          {...labelProps}
          role="radiogroup"
          className="grid w-full grid-cols-3 gap-2"
        >
          {APP_ICON_OPTIONS.map((option) => {
            const selected = option === value;
            const previewName = resolveAppIconName(option, appIdentifier);
            return (
              <button
                key={option}
                type="button"
                role="radio"
                aria-checked={selected}
                aria-label={labels[option]}
                className={cn([
                  "focus-visible:ring-ring flex min-w-0 flex-col items-center gap-1 rounded-xl border p-2 text-xs transition-colors focus-visible:ring-2 focus-visible:outline-none",
                  selected
                    ? "border-primary bg-primary/5 text-foreground"
                    : "border-border bg-card hover:bg-accent",
                ])}
                onClick={() => {
                  void applyAppIconPreference(option);
                  setAppIcon(option);
                }}
              >
                <picture>
                  <source
                    media="(prefers-color-scheme: dark)"
                    srcSet={`/assets/app-icons/${previewName}-dark.png`}
                  />
                  <img
                    src={`/assets/app-icons/${previewName}-light.png`}
                    alt=""
                    className="size-12 rounded-xl"
                  />
                </picture>
                <span className="w-full truncate">{labels[option]}</span>
              </button>
            );
          })}
        </div>
      )}
    </SettingRow>
  );
}
