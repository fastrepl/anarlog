import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";

const ANARLOG_ICON_SRC = "/assets/anarlog-icon.png";

export function TrialDialogIcon({ state }: { state: "started" | "ended" }) {
  const isStarted = state === "started";

  return (
    <div
      {...stylex.props([
        styles.root,
        isStarted ? styles.startedShadow : styles.endedShadow,
      ])}
    >
      <div
        aria-hidden="true"
        {...stylex.props([
          styles.glow,
          isStarted ? styles.startedGlow : styles.endedGlow,
        ])}
      />
      <div {...stylex.props(styles.iconFrame)}>
        <img
          src={ANARLOG_ICON_SRC}
          alt=""
          aria-hidden="true"
          {...stylex.props([
            styles.icon,
            isStarted ? styles.startedIcon : styles.endedIcon,
          ])}
        />
        <div aria-hidden="true" {...stylex.props(styles.overlayFrame)}>
          {isStarted ? (
            <span {...stylex.props(styles.shimmer)} />
          ) : (
            <div {...stylex.props(styles.endedOverlay)} />
          )}
        </div>
      </div>
    </div>
  );
}

const shimmer = stylex.keyframes({
  from: {
    transform: "translateX(-100%)",
  },
  to: {
    transform: "translateX(100%)",
  },
});

const styles = stylex.create({
  endedGlow: {
    backgroundColor: `color-mix(in srgb, ${colors.mutedForeground} 30%, transparent)`,
  },
  endedIcon: {
    filter: "brightness(0.54) grayscale(100%)",
    opacity: 0.58,
  },
  endedOverlay: {
    backgroundImage:
      "linear-gradient(to bottom, rgb(23 23 23 / 0.05), rgb(23 23 23 / 0.2), rgb(0 0 0 / 0.42))",
    inset: 0,
    position: "absolute",
  },
  endedShadow: {
    filter: "drop-shadow(0 14px 22px rgb(0 0 0 / 0.18))",
  },
  glow: {
    borderRadius: "12px",
    filter: "blur(12px)",
    height: "3.5rem",
    position: "absolute",
    width: "3.5rem",
  },
  icon: {
    borderRadius: "12px",
    height: "100%",
    objectFit: "cover",
    objectPosition: "center",
    width: "100%",
  },
  iconFrame: {
    borderRadius: "12px",
    boxShadow:
      "0 1px 0 rgb(255 255 255 / 0.75), 0 10px 24px -10px rgb(0 0 0 / 0.58)",
    height: "3.5rem",
    position: "relative",
    width: "3.5rem",
  },
  overlayFrame: {
    borderRadius: "12px",
    inset: 0,
    overflow: "hidden",
    pointerEvents: "none",
    position: "absolute",
  },
  root: {
    alignItems: "center",
    display: "flex",
    height: "3.5rem",
    justifyContent: "center",
    overflow: "visible",
    position: "relative",
    width: "3.5rem",
  },
  shimmer: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: shimmer,
    backgroundImage:
      "linear-gradient(to right, transparent, rgb(255 255 255 / 0.7), transparent)",
    inset: 0,
    position: "absolute",
    transform: "translateX(-100%)",
  },
  startedGlow: {
    backgroundColor: "rgb(253 230 138 / 0.55)",
  },
  startedIcon: {
    filter: "drop-shadow(0 0 10px rgb(245 158 11 / 0.35))",
  },
  startedShadow: {
    filter: "drop-shadow(0 14px 22px rgb(180 83 9 / 0.22))",
  },
});
