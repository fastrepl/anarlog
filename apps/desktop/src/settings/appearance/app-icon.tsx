import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch, LockSimple } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { getIdentifier } from "@tauri-apps/api/app";
import { platform } from "@tauri-apps/plugin-os";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";

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
    <section {...stylex.props(styles.section)}>
      <div>
        <h3 {...stylex.props(styles.heading)}>
          <Trans>App icon</Trans>
        </h3>
        <p {...stylex.props(styles.description)}>
          <Trans>Choose how Anarlog appears in the Dock.</Trans>
        </p>
      </div>
      <div
        role="radiogroup"
        aria-label={t`App icon`}
        {...stylex.props(styles.options)}
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
              data-app-icon-option
              role="radio"
              aria-checked={selected}
              aria-disabled={locked}
              aria-label={labels[option]}
              title={labels[option]}
              disabled={locked && billing.isUpgradingToPro}
              {...stylex.props(
                styles.option,
                selected ? styles.selectedOption : styles.unselectedOption,
              )}
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
                {...stylex.props(
                  styles.selectedShadow,
                  selected ? styles.visible : styles.hidden,
                )}
              />
              <span
                {...stylex.props(
                  styles.iconFrame,
                  selected && styles.selectedIconFrame,
                )}
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
                      {...stylex.props(styles.preview)}
                    />
                  </picture>
                ) : (
                  <img
                    src={`/assets/app-icons/${previewName}${
                      hasDarkVariant ? `-${theme}` : ""
                    }.png`}
                    alt=""
                    draggable={false}
                    {...stylex.props(styles.preview)}
                  />
                )}
              </span>
              {locked ? (
                <span {...stylex.props(styles.lockBadge)}>
                  {billing.isUpgradingToPro ? (
                    <CircleNotch
                      {...stylex.props(styles.spinner)}
                      aria-hidden
                    />
                  ) : (
                    <LockSimple
                      {...stylex.props(styles.lockIcon)}
                      aria-hidden
                    />
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

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    marginTop: "0.25rem",
  },
  heading: {
    fontSize: "1.125rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
  },
  hidden: {
    opacity: 0,
  },
  iconFrame: {
    borderRadius: "18px",
    display: "flex",
    height: "4rem",
    overflow: "hidden",
    transform: "translateY(0)",
    transitionDuration: "150ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "4rem",
  },
  lockBadge: {
    alignItems: "center",
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    bottom: "0.125rem",
    boxShadow: shadows.sm,
    color: colors.mutedForeground,
    display: "flex",
    height: "1.25rem",
    justifyContent: "center",
    pointerEvents: "none",
    position: "absolute",
    right: "0.125rem",
    width: "1.25rem",
  },
  lockIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  option: {
    alignItems: "center",
    backgroundColor: "transparent",
    borderRadius: "22px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.background}, 0 0 0 4px ${colors.ring}`,
    },
    color: colors.foreground,
    cursor: {
      default: "pointer",
      ":disabled": "wait",
    },
    display: "flex",
    justifyContent: "center",
    padding: "0.125rem",
    position: "relative",
    transform: {
      default: "scale(1)",
      ":active": "scale(0.98)",
    },
    transitionDuration: "150ms",
    transitionProperty: "border-color, scale",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  options: {
    display: "flex",
    flexWrap: "wrap",
    gap: "0.75rem",
  },
  preview: {
    height: "4rem",
    transform: {
      default: "scale(1.16)",
      ":is([data-app-icon-option]:hover *)": "scale(1.21)",
    },
    transitionDuration: "150ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    userSelect: "none",
    width: "4rem",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  selectedIconFrame: {
    transform: "translateY(-0.25rem)",
  },
  selectedOption: {
    borderColor: "transparent",
  },
  selectedShadow: {
    backgroundColor: {
      default: "rgb(0 0 0 / 0.35)",
      ":is(.dark *)": "rgb(255 255 255 / 0.25)",
    },
    borderRadius: radii.full,
    bottom: 0,
    filter: "blur(6px)",
    height: "0.5rem",
    left: "0.625rem",
    pointerEvents: "none",
    position: "absolute",
    right: "0.625rem",
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.75rem",
    width: "0.75rem",
  },
  unselectedOption: {
    borderColor: {
      default: colors.border,
      ":hover": `color-mix(in oklab, ${colors.foreground} 30%, transparent)`,
    },
  },
  visible: {
    opacity: 1,
  },
});
