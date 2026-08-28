import { CheckCircle, XCircle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";

import type { PlanFeature } from "./tiers";

export function PlanFeatureList({
  features,
  dense = false,
}: {
  features: PlanFeature[];
  dense?: boolean;
}) {
  return (
    <div {...stylex.props(styles.list, dense && styles.denseList)}>
      {features.map((feature) => {
        const Icon = feature.included ? CheckCircle : XCircle;

        return (
          <div
            key={feature.label}
            {...stylex.props(styles.feature, dense && styles.denseFeature)}
          >
            <div
              {...stylex.props(
                styles.iconContainer,
                dense && styles.denseIconContainer,
              )}
            >
              <Icon
                {...stylex.props(
                  styles.icon,
                  dense && styles.denseIcon,
                  feature.included ? styles.included : styles.excluded,
                )}
              />
            </div>
            <div {...stylex.props(styles.content)}>
              <div
                {...stylex.props(
                  styles.labelRow,
                  dense && styles.denseLabelRow,
                )}
              >
                <span
                  {...stylex.props(
                    styles.label,
                    dense && styles.denseLabel,
                    feature.included
                      ? styles.includedLabel
                      : styles.excludedLabel,
                  )}
                >
                  {feature.label}
                </span>
              </div>
              {feature.tooltip && !dense && (
                <div {...stylex.props(styles.tooltip)}>{feature.tooltip}</div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

const styles = stylex.create({
  list: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  denseList: {
    gap: "0.375rem",
  },
  feature: {
    alignItems: "flex-start",
    display: "flex",
    gap: "0.75rem",
  },
  denseFeature: {
    gap: "0.375rem",
  },
  iconContainer: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "1.25rem",
  },
  denseIconContainer: {
    height: "1rem",
  },
  icon: {
    height: "1.125rem",
    width: "1.125rem",
  },
  denseIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  included: {
    color: "light-dark(#059669, #34d399)",
  },
  excluded: {
    color: "light-dark(#ef4444, #f87171)",
  },
  content: {
    flexGrow: 1,
  },
  labelRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minHeight: "1.25rem",
  },
  denseLabelRow: {
    minHeight: "1rem",
  },
  label: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  denseLabel: {
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  includedLabel: {
    color: colors.foreground,
  },
  excludedLabel: {
    color: colors.mutedForeground,
  },
  tooltip: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontStyle: "italic",
    lineHeight: "1rem",
    marginTop: "0.125rem",
  },
});
