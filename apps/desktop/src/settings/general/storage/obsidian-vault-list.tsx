import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { displayPath } from "./path-utils";

export function ObsidianVaultList({
  vaults,
  home,
  disabled,
  onSelect,
  actionLabel,
}: {
  vaults: Array<{ path: string }>;
  home: string | undefined;
  disabled?: boolean;
  onSelect: (path: string) => void;
  actionLabel?: string;
}) {
  if (vaults.length === 0) return null;

  return (
    <div {...stylex.props(styles.list)}>
      <p {...stylex.props(styles.heading)}>Detected Obsidian vaults</p>
      {vaults.map((vault) => (
        <button
          key={vault.path}
          disabled={disabled}
          onClick={() => onSelect(vault.path)}
          {...stylex.props(styles.vault)}
        >
          <img
            src="/assets/obsidian-icon.svg"
            {...stylex.props(styles.icon)}
            aria-hidden="true"
          />
          <span {...stylex.props(styles.path)}>
            {displayPath(vault.path, home)}
          </span>
          {actionLabel && (
            <span {...stylex.props(styles.action)}>{actionLabel}</span>
          )}
        </button>
      ))}
    </div>
  );
}

const styles = stylex.create({
  action: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  heading: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
  },
  icon: {
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  list: {
    display: "flex",
    flexDirection: "column",
    gap: "0.375rem",
  },
  path: {
    flex: "1",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  vault: {
    alignItems: "center",
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
});
