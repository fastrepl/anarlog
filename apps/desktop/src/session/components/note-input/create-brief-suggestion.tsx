import { useLingui } from "@lingui/react/macro";
import { Sparkle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

export function CreateBriefSuggestion({ onCreate }: { onCreate: () => void }) {
  const { t } = useLingui();
  const label = t`Create a brief to prepare this meeting`;

  return (
    <button
      type="button"
      aria-label={label}
      onClick={onCreate}
      {...stylex.props(styles.button)}
    >
      <Sparkle aria-hidden {...stylex.props(styles.icon)} />
      <span {...stylex.props(styles.label)}>{label}</span>
    </button>
  );
}

const styles = stylex.create({
  button: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":focus-visible": colors.accent,
      ":hover": colors.accent,
    },
    borderRadius: radii.md,
    color: {
      default: colors.mutedForeground,
      ":focus-visible": colors.foreground,
      ":hover": colors.foreground,
    },
    display: "flex",
    gap: "0.5rem",
    height: "2rem",
    marginBottom: "1.5rem",
    marginLeft: "-0.5rem",
    maxWidth: "100%",
    outlineStyle: {
      default: null,
      ":focus-visible": "none",
    },
    paddingInline: "0.5rem",
    pointerEvents: "auto",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "fit-content",
  },
  icon: {
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  label: {
    fontSize: "0.875rem",
    fontWeight: 500,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});

export { styles as createBriefSuggestionStyles };
