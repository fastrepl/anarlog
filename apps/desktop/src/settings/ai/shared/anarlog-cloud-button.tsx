import * as stylex from "@stylexjs/stylex";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";

export function AnlgProviderRow({ children }: { children: React.ReactNode }) {
  return <div {...stylex.props(styles.providerRow)}>{children}</div>;
}

export function AnlgCloudCTAButton({
  isPaid,
  canStartTrial,
  highlight,
  onClick,
}: {
  isPaid: boolean;
  canStartTrial: boolean | undefined;
  highlight?: boolean;
  onClick: () => void;
}) {
  const buttonLabel = isPaid
    ? "Ready to use"
    : canStartTrial
      ? "Start Free Trial"
      : "Upgrade";

  const showShimmer = highlight && !isPaid;

  return (
    <button
      onClick={onClick}
      {...stylex.props(
        styles.button,
        isPaid ? styles.paidButton : styles.upgradeButton,
      )}
    >
      {showShimmer && <div {...stylex.props(styles.shimmer)} />}
      <span {...stylex.props(styles.buttonLabel)}>{buttonLabel}</span>
    </button>
  );
}

const shimmer = stylex.keyframes({
  from: { transform: "translateX(-100%)" },
  to: { transform: "translateX(100%)" },
});

const styles = stylex.create({
  button: {
    borderRadius: radii.full,
    fontFamily: fonts.mono,
    fontSize: "0.75rem",
    height: "2.125rem",
    lineHeight: "1rem",
    overflow: "hidden",
    paddingInline: "1rem",
    position: "relative",
    textAlign: "center",
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "fit-content",
  },
  buttonLabel: {
    position: "relative",
  },
  paidButton: {
    backgroundImage: `linear-gradient(to top, ${colors.muted}, ${colors.accent})`,
    boxShadow: {
      default: "0 1px 2px rgb(0 0 0 / 0.05)",
      ":hover":
        "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    },
    color: colors.foreground,
  },
  providerRow: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
  },
  shimmer: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: shimmer,
    backgroundImage:
      "linear-gradient(to right, transparent, rgb(255 255 255 / 0.2), transparent)",
    inset: 0,
    position: "absolute",
    transform: "translateX(-100%)",
  },
  upgradeButton: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    boxShadow: {
      default:
        "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
      ":hover":
        "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)",
    },
    color: colors.primaryForeground,
    transform: {
      default: "scale(1)",
      ":active": "scale(0.98)",
      ":hover": "scale(1.02)",
    },
  },
});
