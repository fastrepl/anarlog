import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";

import { useTabs } from "~/store/zustand/tabs";

export function ConfigError() {
  const openNew = useTabs((state) => state.openNew);

  return (
    <div role="alert" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.copy)}>
        <p {...stylex.props(styles.title)}>
          <Trans>Set up AI summaries</Trans>
        </p>
        <p {...stylex.props(styles.description)}>
          <Trans>
            Start a Pro trial or add your own LLM API key to generate a summary
            from this transcript.
          </Trans>
        </p>
      </div>
      <div {...stylex.props(styles.actions)}>
        <Button
          sx={styles.button}
          onClick={() =>
            openNew({ type: "settings", state: { tab: "account" } })
          }
        >
          <Trans>Get Pro</Trans>
        </Button>
        <Button
          variant="outline"
          sx={styles.button}
          onClick={() =>
            openNew({ type: "settings", state: { tab: "intelligence" } })
          }
        >
          <Trans>Add API key</Trans>
        </Button>
      </div>
    </div>
  );
}

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  button: {
    boxShadow: "none",
  },
  copy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    marginBottom: "1.5rem",
    maxWidth: "28rem",
    textAlign: "center",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: 1.625,
  },
  root: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    height: "100%",
    justifyContent: "center",
    minHeight: "400px",
    paddingInline: "1.5rem",
  },
  title: {
    fontSize: "1rem",
    fontWeight: 500,
  },
});
