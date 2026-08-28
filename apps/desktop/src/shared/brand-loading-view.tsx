import * as stylex from "@stylexjs/stylex";

import { colors, media } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { AnarlogMark } from "./anarlog-mark";

const shimmer = stylex.keyframes({
  from: {
    maskPosition: "120% 0%",
  },
  to: {
    maskPosition: "-20% 0%",
  },
});

export function BrandLoadingView({ detail }: { detail?: string }) {
  return (
    <div
      role="status"
      aria-label="Loading"
      data-tauri-drag-region
      {...stylex.props(styles.root)}
    >
      <div {...stylex.props(styles.content)}>
        <div {...stylex.props(styles.mark)}>
          <AnarlogMark sx={styles.baseMark} />
          <div
            aria-hidden="true"
            {...mergeStyleXProps(styles.shimmer, undefined, {
              WebkitMaskSize: "220% 100%",
            })}
          >
            <AnarlogMark sx={styles.fullWidth} />
          </div>
        </div>
        {detail ? <p {...stylex.props(styles.detail)}>{detail}</p> : null}
      </div>
    </div>
  );
}

const styles = stylex.create({
  baseMark: {
    color: `color-mix(in oklab, ${colors.foreground} 20%, transparent)`,
    width: "100%",
  },
  content: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
  },
  detail: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "1.25rem",
    maxWidth: "16rem",
    textAlign: "center",
  },
  fullWidth: {
    width: "100%",
  },
  mark: {
    filter: {
      default: "drop-shadow(0 10px 24px rgba(0, 0, 0, 0.1))",
      ":is(.dark *)": "drop-shadow(0 12px 28px rgba(0, 0, 0, 0.45))",
    },
    position: "relative",
    width: "4.5rem",
  },
  root: {
    alignItems: "center",
    backgroundColor: colors.background,
    display: "flex",
    height: "100vh",
    justifyContent: "center",
    width: "100vw",
  },
  shimmer: {
    animationDuration: {
      default: "2.4s",
      [media.reducedMotion]: "0s",
    },
    animationIterationCount: "infinite",
    animationName: {
      default: shimmer,
      [media.reducedMotion]: "none",
    },
    animationTimingFunction: "ease-in-out",
    color: {
      default: `color-mix(in oklab, ${colors.foreground} 75%, transparent)`,
      ":is(.dark *)": `color-mix(in oklab, ${colors.foreground} 50%, transparent)`,
    },
    inset: 0,
    maskImage:
      "linear-gradient(105deg, transparent 36%, #000 50%, transparent 64%)",
    maskSize: "220% 100%",
    opacity: {
      default: 1,
      [media.reducedMotion]: 0,
    },
    pointerEvents: "none",
    position: "absolute",
  },
});
