import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@tanstack/react-query";

import { colors, fonts, media } from "@anlg/design-system/tokens.stylex";
import { commands as windowsCommands } from "@anlg/plugin-windows";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { commands } from "~/types/tauri.gen";

export function DevtoolsSection() {
  const enabledQuery = useQuery({
    queryKey: ["devtools-panel", "enabled"],
    queryFn: commands.showDevtool,
    staleTime: Infinity,
  });
  const openMutation = useMutation({
    mutationFn: async () => {
      const result = await windowsCommands.devtoolsPanelShow();
      if (result.status === "error") {
        throw new Error(result.error);
      }
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  if (enabledQuery.data !== true) {
    return null;
  }

  return (
    <section {...stylex.props(styles.section)}>
      <h2 {...stylex.props(styles.heading)}>{t`Devtools`}</h2>
      <div {...stylex.props(styles.row)}>
        <div {...stylex.props(styles.copy)}>
          <h3 {...stylex.props(styles.title)}>{t`Devtools panel`}</h3>
          <p {...stylex.props(styles.description)}>
            <Trans>
              Preview notifications, toasts, updates, and billing dialogs. Only
              available in dev and staging builds.
            </Trans>
          </p>
        </div>
        <div {...stylex.props(styles.action)}>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={openMutation.isPending}
            onClick={() => openMutation.mutate()}
          >
            {t`Open panel`}
          </Button>
        </div>
      </div>
    </section>
  );
}

const styles = stylex.create({
  action: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
  },
  copy: {
    minWidth: 0,
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    marginTop: "0.25rem",
  },
  heading: {
    fontFamily: fonts.sans,
    fontSize: "1.125rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
  },
  row: {
    alignItems: {
      default: "stretch",
      [media.sm]: "flex-start",
    },
    display: "flex",
    flexDirection: {
      default: "column",
      [media.sm]: "row",
    },
    gap: "1rem",
    justifyContent: {
      default: "flex-start",
      [media.sm]: "space-between",
    },
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  title: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
});
