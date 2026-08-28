import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";

import { fonts } from "@anlg/design-system/tokens.stylex";

import {
  LegacyMigrationCleanupRow,
  useLegacyMigrationCleanup,
} from "./legacy-cleanup";

export function StorageSettingsView() {
  const { visible } = useLegacyMigrationCleanup();
  if (!visible) return null;

  return (
    <div>
      <h2 {...stylex.props(styles.title)}>
        <Trans>Storage</Trans>
      </h2>
      <div {...stylex.props(styles.content)}>
        <LegacyMigrationCleanupRow />
      </div>
    </div>
  );
}

const styles = stylex.create({
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  title: {
    fontFamily: fonts.sans,
    fontSize: "1.125rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
    marginBottom: "1rem",
  },
});
