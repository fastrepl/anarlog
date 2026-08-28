import { useLingui } from "@lingui/react/macro";
import { MagnifyingGlass, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

export function filterProviders<T extends { id: string; displayName: string }>(
  providers: readonly T[],
  query: string,
): T[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) {
    return [...providers];
  }

  return providers.filter((provider) =>
    `${provider.displayName} ${provider.id}`
      .toLocaleLowerCase()
      .includes(normalizedQuery),
  );
}

export function ProviderSearch({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const { t } = useLingui();

  return (
    <div {...stylex.props(styles.container)}>
      <MagnifyingGlass {...stylex.props(styles.icon)} />
      <input
        type="search"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") onChange("");
        }}
        placeholder={t`Search providers...`}
        aria-label={t`Search providers`}
        {...stylex.props(styles.input)}
      />
      {value ? (
        <button
          type="button"
          onClick={() => onChange("")}
          {...stylex.props(styles.clearButton)}
          aria-label={t`Clear search`}
        >
          <X {...stylex.props(styles.clearIcon)} />
        </button>
      ) : null}
    </div>
  );
}

const styles = stylex.create({
  clearButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    flexShrink: 0,
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  clearIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  container: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.muted} 50%, transparent)`,
      ":focus-within": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    height: "2rem",
    marginLeft: "auto",
    maxWidth: "55%",
    paddingInline: "0.625rem",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "14rem",
  },
  icon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "0.875rem",
    width: "0.875rem",
  },
  input: {
    backgroundColor: "transparent",
    color: {
      default: null,
      "::placeholder": colors.mutedForeground,
    },
    display: {
      default: null,
      "::-webkit-search-cancel-button": "none",
    },
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    outline: {
      default: null,
      ":focus": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus": "2px",
    },
  },
});
