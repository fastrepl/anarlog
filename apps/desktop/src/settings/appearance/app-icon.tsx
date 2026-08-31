import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch, LockSimple } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { platform } from "@tauri-apps/plugin-os";

import { cn } from "@anlg/utils";

import { useBillingAccess } from "~/auth/billing-context";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import {
  type AppIconPreference,
  hasDarkAppIconVariant,
  normalizeAppIconPreference,
  resolveAppIconName,
} from "~/shared/theme/icon";
import { applyAppIconPreference } from "~/shared/theme/provider";
import type { ThemePreference } from "~/shared/theme/resolve";

const APP_ICON_OPTIONS = [
  "default",
  "stable",
  "anagram",
  "dev",
  "staging",
  "journal",
  "notepad",
  "stone",
  "typewriter-key",
  "walnut",
] as const satisfies readonly AppIconPreference[];

const PREVIEW_CLASS =
  "size-16 scale-[1.16] transition-transform duration-150 select-none group-hover:scale-[1.21]";

export function AppIconSelector() {
  const { t } = useLingui();
  const billing = useBillingAccess();
  const value = normalizeAppIconPreference(useConfigValue("app_icon"));
  const storedTheme = useConfigValue("theme") as ThemePreference;
  const theme: ThemePreference =
    storedTheme === "light" || storedTheme === "dark" ? storedTheme : "system";
  const setAppIcon = useSetSettingValue("app_icon");
  const { data: appIdentifier = "com.hyprnote.stable" } = useQuery({
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
    journal: t`Field Journal`,
    notepad: t`Notepad`,
    stone: t`Stone`,
    "typewriter-key": t`Typewriter Key`,
    walnut: t`Walnut`,
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
        className="flex flex-wrap gap-3"
      >
        {options.map((option) => {
          const selected =
            resolveAppIconName(option, appIdentifier) === selectedIconName;
          const previewName = resolveAppIconName(option, appIdentifier);
          const hasDarkVariant = hasDarkAppIconVariant(previewName);
          const locked = !billing.isPro && !selected;

          return (
            <button
              key={option}
              type="button"
              role="radio"
              aria-checked={selected}
              aria-disabled={locked}
              aria-label={labels[option]}
              title={labels[option]}
              disabled={locked && billing.isUpgradingToPro}
              className={cn([
                "group text-foreground focus-visible:ring-ring focus-visible:ring-offset-background relative flex cursor-pointer items-center justify-center rounded-[22px] bg-transparent p-0.5 pb-2.5 transition-transform duration-150 focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none active:scale-[0.98] disabled:cursor-wait",
              ])}
              onClick={() => {
                if (locked) {
                  billing.upgradeToPro();
                  return;
                }

                void applyAppIconPreference(option, theme);
                setAppIcon(option);
              }}
            >
              <span
                aria-hidden
                data-app-icon-stage=""
                className={cn([
                  "rounded-pill pointer-events-none absolute bottom-0.5 left-1/2 h-4 w-14 -translate-x-1/2",
                  "bg-sky-400/60 blur-md dark:bg-sky-300/55",
                  "transition-opacity duration-150",
                  selected ? "opacity-100" : "opacity-0",
                ])}
              />
              <span
                aria-hidden
                className={cn([
                  "rounded-pill pointer-events-none absolute bottom-1.5 left-1/2 h-1.5 w-8 -translate-x-1/2",
                  "bg-sky-300/90 blur-[3px] dark:bg-sky-200/80",
                  "transition-opacity duration-150",
                  selected ? "opacity-100" : "opacity-0",
                ])}
              />
              <span
                className={cn([
                  "relative flex size-16 overflow-hidden rounded-[18px]",
                  "transition-transform duration-150",
                  selected && "-translate-y-1.5",
                ])}
              >
                {theme === "system" && hasDarkVariant ? (
                  <picture>
                    <source
                      media="(prefers-color-scheme: dark)"
                      srcSet={`/assets/app-icons/${previewName}-dark.png`}
                    />
                    <img
                      src={`/assets/app-icons/${previewName}-light.png`}
                      alt=""
                      draggable={false}
                      className={PREVIEW_CLASS}
                    />
                  </picture>
                ) : (
                  <img
                    src={`/assets/app-icons/${previewName}${
                      hasDarkVariant ? `-${theme}` : ""
                    }.png`}
                    alt=""
                    draggable={false}
                    className={PREVIEW_CLASS}
                  />
                )}
              </span>
              {locked ? (
                <span className="bg-background border-border text-muted-foreground pointer-events-none absolute right-1 bottom-3 flex size-5 items-center justify-center rounded-full border shadow-sm">
                  {billing.isUpgradingToPro ? (
                    <CircleNotch className="size-3 animate-spin" aria-hidden />
                  ) : (
                    <LockSimple className="size-3" aria-hidden />
                  )}
                </span>
              ) : null}
            </button>
          );
        })}
      </div>
    </section>
  );
}
