import { t } from "@lingui/core/macro";
import { CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";

import { useStoredSettingValuesQuery } from "~/settings/queries";

export function SettingsHydrationBoundary({
  children,
}: {
  children: ReactNode;
}) {
  const { data, isLoading, error } = useStoredSettingValuesQuery();

  if (error) {
    throw error;
  }
  if (isLoading || !data) {
    return (
      <div {...stylex.props(styles.loading)}>
        <CircleNotch
          aria-label={t`Loading settings`}
          {...stylex.props(styles.spinner)}
        />
      </div>
    );
  }

  return children;
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  loading: {
    alignItems: "center",
    display: "flex",
    justifyContent: "center",
    minHeight: "12rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    color: colors.mutedForeground,
    height: "1.25rem",
    width: "1.25rem",
  },
});
