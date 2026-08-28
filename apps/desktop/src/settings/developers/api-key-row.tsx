import { t } from "@lingui/core/macro";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";

export function ApiKeyRow({
  apiKey,
  onRevoke,
}: {
  apiKey: {
    name: string;
    key_prefix: string;
    created_at: string;
    last_used_at: string | null;
  };
  onRevoke: () => void;
}) {
  return (
    <li {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.identity)}>
        <code {...stylex.props(styles.prefix)}>{apiKey.key_prefix}…</code>
        <span {...stylex.props(styles.name)}>{apiKey.name}</span>
      </div>
      <div {...stylex.props(styles.actions)}>
        <span {...stylex.props(styles.date)}>
          {apiKey.last_used_at
            ? t`Last used ${apiKey.last_used_at.slice(0, 10)}`
            : t`Created ${apiKey.created_at.slice(0, 10)}`}
        </span>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          sx={styles.revoke}
          onClick={onRevoke}
        >
          {t`Revoke`}
        </Button>
      </div>
    </li>
  );
}

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.5rem",
  },
  date: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  identity: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
  },
  name: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  prefix: {
    backgroundColor: colors.muted,
    borderRadius: radii.md,
    flexShrink: 0,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  revoke: {
    color: colors.destructive,
    height: "1.75rem",
  },
  row: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.75rem",
    justifyContent: "space-between",
    lineHeight: "1.25rem",
  },
});
