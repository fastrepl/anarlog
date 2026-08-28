import { Trans, useLingui } from "@lingui/react/macro";
import { Check } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, media, radii } from "@anlg/design-system/tokens.stylex";

import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { normalizeAppIconPreference } from "~/shared/theme/icon";
import { applyThemePreference } from "~/shared/theme/provider";
import type { ThemePreference } from "~/shared/theme/resolve";

const THEME_OPTIONS = [
  "light",
  "dark",
  "system",
] as const satisfies readonly ThemePreference[];

export function ThemeSelector() {
  const { t } = useLingui();
  const storedValue = useConfigValue("theme") as ThemePreference;
  const value = THEME_OPTIONS.includes(storedValue) ? storedValue : "system";
  const appIcon = normalizeAppIconPreference(useConfigValue("app_icon"));
  const setTheme = useSetSettingValue("theme");
  const options = [
    { value: "light", label: t`Light`, description: t`Bright canvas` },
    { value: "dark", label: t`Dark`, description: t`Low-light canvas` },
    { value: "system", label: t`System`, description: t`Match your device` },
  ] as const satisfies readonly {
    value: ThemePreference;
    label: string;
    description: string;
  }[];

  return (
    <section {...stylex.props(styles.section)}>
      <div>
        <h3 {...stylex.props(styles.heading)}>
          <Trans>Theme</Trans>
        </h3>
        <p {...stylex.props(styles.description)}>
          <Trans>Choose how Anarlog looks on this device.</Trans>
        </p>
      </div>
      <div
        role="radiogroup"
        aria-label={t`Color theme`}
        {...stylex.props(styles.options)}
      >
        {options.map((option) => {
          const selected = value === option.value;

          return (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={selected}
              {...stylex.props(
                styles.option,
                selected ? styles.selectedOption : styles.unselectedOption,
              )}
              onClick={() => {
                void applyThemePreference(option.value, appIcon);
                setTheme(option.value);
              }}
            >
              <ThemePreview theme={option.value} />
              <div {...stylex.props(styles.optionFooter)}>
                <div {...stylex.props(styles.optionCopy)}>
                  <div {...stylex.props(styles.optionLabel)}>
                    {option.label}
                  </div>
                  <div {...stylex.props(styles.optionDescription)}>
                    {option.description}
                  </div>
                </div>
                <span
                  aria-hidden="true"
                  {...stylex.props(
                    styles.checkmark,
                    selected ? styles.visibleCheckmark : styles.hiddenCheckmark,
                  )}
                >
                  <Check {...stylex.props(styles.checkIcon)} weight="bold" />
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </section>
  );
}

function ThemePreview({ theme }: { theme: ThemePreference }) {
  return (
    <div aria-hidden="true" {...stylex.props(styles.preview)}>
      <PreviewCanvas dark={theme === "dark"} />
      {theme === "system" ? (
        <div {...stylex.props(styles.systemPreview)}>
          <PreviewCanvas dark />
        </div>
      ) : null}
    </div>
  );
}

function PreviewCanvas({ dark }: { dark: boolean }) {
  return (
    <div
      {...stylex.props(
        styles.previewCanvas,
        dark ? styles.darkCanvas : styles.lightCanvas,
      )}
    >
      <div {...stylex.props(styles.windowControls)}>
        <span {...stylex.props(styles.windowControl, styles.redControl)} />
        <span {...stylex.props(styles.windowControl, styles.amberControl)} />
        <span {...stylex.props(styles.windowControl, styles.greenControl)} />
      </div>
      <div {...stylex.props(styles.previewContent)}>
        <div
          {...stylex.props(
            styles.previewSidebar,
            dark ? styles.darkSidebar : styles.lightSidebar,
          )}
        />
        <div {...stylex.props(styles.previewLines)}>
          <span
            {...stylex.props(
              styles.previewHeadingLine,
              dark ? styles.darkHeadingLine : styles.lightHeadingLine,
            )}
          />
          <span
            {...stylex.props(
              styles.previewLine,
              styles.longLine,
              dark ? styles.darkLine : styles.lightLine,
            )}
          />
          <span
            {...stylex.props(
              styles.previewLine,
              styles.mediumLine,
              dark ? styles.darkLine : styles.lightLine,
            )}
          />
          <span
            {...stylex.props(
              styles.previewLine,
              styles.shortLine,
              dark ? styles.darkLine : styles.lightLine,
            )}
          />
        </div>
      </div>
    </div>
  );
}

const styles = stylex.create({
  amberControl: {
    backgroundColor: "rgb(251 191 36)",
  },
  checkIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  checkmark: {
    alignItems: "center",
    backgroundColor: colors.foreground,
    borderRadius: radii.full,
    color: colors.background,
    display: "flex",
    flexShrink: 0,
    height: "1.25rem",
    justifyContent: "center",
    transitionDuration: "150ms",
    transitionProperty: "filter, opacity, scale",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.25rem",
  },
  darkCanvas: {
    backgroundColor: "rgb(10 10 10)",
    color: "rgb(245 245 245)",
  },
  darkHeadingLine: {
    backgroundColor: "rgb(212 212 212)",
  },
  darkLine: {
    backgroundColor: "rgb(64 64 64)",
  },
  darkSidebar: {
    backgroundColor: "rgb(38 38 38)",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    marginTop: "0.25rem",
  },
  greenControl: {
    backgroundColor: "rgb(74 222 128)",
  },
  heading: {
    fontSize: "1.125rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
  },
  hiddenCheckmark: {
    filter: "blur(4px)",
    opacity: 0,
    scale: 0.25,
  },
  lightCanvas: {
    backgroundColor: "white",
    color: "rgb(23 23 23)",
  },
  lightHeadingLine: {
    backgroundColor: "rgb(64 64 64)",
  },
  lightLine: {
    backgroundColor: "rgb(229 229 229)",
  },
  lightSidebar: {
    backgroundColor: "rgb(245 245 245)",
  },
  longLine: {
    width: "80%",
  },
  mediumLine: {
    width: "60%",
  },
  option: {
    backgroundColor: colors.background,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.foreground,
    cursor: "pointer",
    overflow: "hidden",
    position: "relative",
    textAlign: "left",
    transform: {
      default: "scale(1)",
      ":active": "scale(0.98)",
    },
    transitionDuration: "150ms",
    transitionProperty: "background-color, border-color, box-shadow, scale",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  optionCopy: {
    flex: "1",
    minWidth: 0,
  },
  optionDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  optionFooter: {
    alignItems: "center",
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    padding: "0.75rem",
  },
  optionLabel: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  options: {
    display: "grid",
    gap: "0.75rem",
    gridTemplateColumns: {
      default: "minmax(0, 1fr)",
      [media.sm]: "repeat(3, minmax(0, 1fr))",
    },
  },
  preview: {
    height: "7rem",
    overflow: "hidden",
    position: "relative",
  },
  previewCanvas: {
    display: "flex",
    flexDirection: "column",
    inset: 0,
    padding: "0.75rem",
    position: "absolute",
  },
  previewContent: {
    display: "flex",
    flex: "1",
    gap: "0.75rem",
  },
  previewHeadingLine: {
    borderRadius: radii.full,
    height: "0.375rem",
    width: "2.5rem",
  },
  previewLine: {
    borderRadius: radii.full,
    height: "0.25rem",
  },
  previewLines: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    gap: "0.5rem",
    paddingBlock: "0.25rem",
  },
  previewSidebar: {
    borderRadius: radii.md,
    width: "25%",
  },
  redControl: {
    backgroundColor: "rgb(248 113 113)",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  selectedOption: {
    backgroundColor: `color-mix(in oklab, ${colors.accent} 40%, transparent)`,
    borderColor: `color-mix(in oklab, ${colors.foreground} 50%, transparent)`,
    boxShadow: "0 1px 2px rgb(0 0 0 / 0.05)",
  },
  shortLine: {
    width: "66.666667%",
  },
  systemPreview: {
    clipPath: "polygon(100% 0, 100% 100%, 0 100%)",
    inset: 0,
    position: "absolute",
  },
  unselectedOption: {
    backgroundColor: {
      default: colors.background,
      ":hover": `color-mix(in oklab, ${colors.accent} 20%, transparent)`,
    },
    borderColor: {
      default: colors.border,
      ":hover": `color-mix(in oklab, ${colors.foreground} 30%, transparent)`,
    },
  },
  visibleCheckmark: {
    filter: "none",
    opacity: 1,
    scale: 1,
  },
  windowControl: {
    borderRadius: radii.full,
    height: "0.375rem",
    width: "0.375rem",
  },
  windowControls: {
    display: "flex",
    gap: "0.25rem",
    marginBottom: "0.75rem",
  },
});
