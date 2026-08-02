import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";

import { cn } from "@anlg/utils";

import { useSetSettingValue } from "~/settings/queries";
import { SettingRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";
import {
  type AppIconPreference,
  normalizeAppIconPreference,
} from "~/shared/theme/icon";
import { applyAppIconPreference } from "~/shared/theme/provider";

const APP_ICON_OPTIONS = [
  {
    value: "default",
    lightPreview: "/assets/app-icons/default-light.png",
    darkPreview: "/assets/app-icons/default-dark.png",
  },
  {
    value: "anagram",
    lightPreview: "/assets/app-icons/anagram-light.png",
    darkPreview: "/assets/app-icons/anagram-dark.png",
  },
] as const satisfies readonly {
  value: AppIconPreference;
  lightPreview: string;
  darkPreview: string;
}[];

export function AppIconSelector() {
  const { t } = useLingui();
  const value = normalizeAppIconPreference(useConfigValue("app_icon"));
  const setAppIcon = useSetSettingValue("app_icon");
  const labels = {
    default: t`Default`,
    anagram: t`Anagram`,
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
          className="grid w-full grid-cols-2 gap-2"
        >
          {APP_ICON_OPTIONS.map((option) => {
            const selected = option.value === value;
            return (
              <button
                key={option.value}
                type="button"
                role="radio"
                aria-checked={selected}
                aria-label={labels[option.value]}
                className={cn([
                  "focus-visible:ring-ring flex min-w-0 flex-col items-center gap-1 rounded-xl border p-2 text-xs transition-colors focus-visible:ring-2 focus-visible:outline-none",
                  selected
                    ? "border-primary bg-primary/5 text-foreground"
                    : "border-border bg-card hover:bg-accent",
                ])}
                onClick={() => {
                  void applyAppIconPreference(option.value);
                  setAppIcon(option.value);
                }}
              >
                <picture>
                  <source
                    media="(prefers-color-scheme: dark)"
                    srcSet={option.darkPreview}
                  />
                  <img
                    src={option.lightPreview}
                    alt=""
                    className="size-12 rounded-xl"
                  />
                </picture>
                <span className="w-full truncate">{labels[option.value]}</span>
              </button>
            );
          })}
        </div>
      )}
    </SettingRow>
  );
}
