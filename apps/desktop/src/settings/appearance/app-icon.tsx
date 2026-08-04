import { Trans, useLingui } from "@lingui/react/macro";
import { Check } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { platform } from "@tauri-apps/plugin-os";

import { cn } from "@anlg/utils";

import { useSetSettingValue } from "~/settings/queries";
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
  const defaultIconName = resolveAppIconName("default", appIdentifier);
  const selectedIconName = resolveAppIconName(value, appIdentifier);
  const options = APP_ICON_OPTIONS.filter(
    (option) => option === "default" || option !== defaultIconName,
  );

  if (platform() !== "macos") {
    return null;
  }

  return (
    <section className="flex flex-col gap-4">
      <div>
        <h3 className="text-lg font-semibold">
          <Trans>App icon</Trans>
        </h3>
        <p className="text-muted-foreground mt-1 text-sm">
          <Trans>Choose how Anarlog appears in the Dock.</Trans>
        </p>
      </div>
      <div
        role="radiogroup"
        aria-label={t`App icon`}
        className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5"
      >
        {options.map((option) => {
          const selected =
            resolveAppIconName(option, appIdentifier) === selectedIconName;
          const previewName = resolveAppIconName(option, appIdentifier);

          return (
            <button
              key={option}
              type="button"
              role="radio"
              aria-checked={selected}
              aria-label={labels[option]}
              className={cn([
                "group bg-background text-foreground focus-visible:ring-ring focus-visible:ring-offset-background relative flex cursor-pointer flex-col items-center gap-3 rounded-2xl border p-4 transition-[background-color,border-color,box-shadow,scale] duration-150 focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none active:scale-[0.98]",
                selected
                  ? "border-foreground/50 bg-accent/40 shadow-xs"
                  : "border-border hover:border-foreground/30 hover:bg-accent/20",
              ])}
              onClick={() => {
                void applyAppIconPreference(option);
                setAppIcon(option);
              }}
            >
              <span
                aria-hidden="true"
                className={cn([
                  "bg-foreground text-background absolute top-2 right-2 flex size-5 items-center justify-center rounded-full transition-[filter,opacity,scale] duration-150",
                  selected
                    ? "scale-100 opacity-100 blur-none"
                    : "scale-25 opacity-0 blur-[4px]",
                ])}
              >
                <Check className="size-3" weight="bold" />
              </span>
              <picture>
                <source
                  media="(prefers-color-scheme: dark)"
                  srcSet={`/assets/app-icons/${previewName}-dark.png`}
                />
                <img
                  src={`/assets/app-icons/${previewName}-light.png`}
                  alt=""
                  draggable={false}
                  className="size-16 transition-transform duration-150 select-none group-hover:scale-105"
                />
              </picture>
              <span className="w-full truncate text-center text-sm font-medium">
                {labels[option]}
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
