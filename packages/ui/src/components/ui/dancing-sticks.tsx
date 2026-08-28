import * as stylex from "@stylexjs/stylex";
import { memo, useMemo } from "react";

import { radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

function mulberry32(seed: number): () => number {
  let value = seed;
  return () => {
    value += 0x6d2b79f5;
    let t = value;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

type DancingSticksProps = {
  color?: string;
  amplitude: number;
  height?: number;
  width?: number;
  stickWidth?: number;
  gap?: number;
} & StyleXProps;

function generatePattern(count: number): number[] {
  if (count <= 1) {
    return [100];
  }

  const pattern: number[] = [];
  const mid = (count - 1) / 2;
  for (let i = 0; i < count; i++) {
    const distance = Math.abs(i - mid) / mid;
    pattern.push(50 + 50 * (1 - distance));
  }
  return pattern;
}

export const DancingSticks = memo(function DancingSticks({
  color = "#e5e5e5",
  amplitude,
  height,
  width,
  stickWidth,
  gap,
  sx,
}: DancingSticksProps) {
  const resolvedHeight = height ?? 16;
  const resolvedStickWidth = stickWidth ?? 2;
  const resolvedGap = gap ?? 1;
  const resolvedWidth = width ?? 17;
  const stickCount = Math.max(
    1,
    Math.floor(
      (resolvedWidth + resolvedGap) / (resolvedStickWidth + resolvedGap),
    ),
  );
  const isFlat = amplitude === 0;
  const pattern = useMemo(() => generatePattern(stickCount), [stickCount]);

  const amplitudeScale = useMemo(() => {
    const clamped = Math.max(0, Math.min(1, amplitude));
    return 0.2 + 0.8 * clamped;
  }, [amplitude]);

  const stickParams = useMemo(
    () =>
      pattern.map((baseLength, index) => {
        const maxScaleY = Math.max(0.25, Math.min(1, baseLength / 100));
        const rng = mulberry32((index + 1) * 10007);
        const speed = 4 + rng() * 3;
        const phase = rng() * Math.PI * 2;
        const durationSeconds = (Math.PI * 2) / speed;
        const delaySeconds = -(phase / (Math.PI * 2)) * durationSeconds;
        return { maxScaleY, durationSeconds, delaySeconds };
      }),
    [pattern],
  );

  if (isFlat) {
    return (
      <div
        {...mergeStyleXProps([
          styles.container,
          styles.containerSize(resolvedHeight, resolvedWidth),
          sx,
        ])}
      >
        <div
          {...stylex.props(
            styles.flatIndicator,
            styles.flatIndicatorAppearance(resolvedWidth, color),
          )}
        />
      </div>
    );
  }

  return (
    <div
      {...mergeStyleXProps([
        styles.container,
        styles.animatedContainer(
          resolvedHeight,
          resolvedWidth,
          resolvedGap,
          `scaleY(${amplitudeScale})`,
        ),
        sx,
      ])}
    >
      {stickParams.map(
        ({ maxScaleY, durationSeconds, delaySeconds }, index) => (
          <div
            key={index}
            {...stylex.props(
              styles.stickContainer,
              styles.stickContainerSize(
                resolvedStickWidth,
                resolvedHeight,
                `scaleY(${maxScaleY})`,
              ),
            )}
          >
            <div
              {...stylex.props(
                styles.stick,
                styles.stickAppearance(
                  resolvedHeight,
                  color,
                  `${durationSeconds}s`,
                  `${delaySeconds}s`,
                ),
              )}
            />
          </div>
        ),
      )}
    </div>
  );
});

const dancingStick = stylex.keyframes({
  "0%": {
    transform: "scaleY(0.2)",
  },
  "50%": {
    transform: "scaleY(1)",
  },
  "100%": {
    transform: "scaleY(0.2)",
  },
});

const styles = stylex.create({
  container: {
    alignItems: "center",
    display: "flex",
    justifyContent: "center",
  },
  containerSize: (height: number, width: number) => ({
    height,
    width,
  }),
  animatedContainer: (
    height: number,
    width: number,
    gap: number,
    transform: string,
  ) => ({
    gap,
    height,
    transform,
    transformOrigin: "center",
    width,
  }),
  flatIndicator: {
    borderRadius: radii.full,
    height: 1,
  },
  flatIndicatorAppearance: (width: number, backgroundColor: string) => ({
    backgroundColor,
    width,
  }),
  stickContainer: {
    alignItems: "center",
    display: "flex",
    justifyContent: "center",
    transformOrigin: "center",
  },
  stickContainerSize: (width: number, height: number, transform: string) => ({
    height,
    transform,
    width,
  }),
  stick: {
    animationIterationCount: "infinite",
    animationName: dancingStick,
    animationTimingFunction: "ease-in-out",
    borderRadius: radii.full,
    transformOrigin: "center",
    width: "100%",
  },
  stickAppearance: (
    height: number,
    backgroundColor: string,
    animationDuration: string,
    animationDelay: string,
  ) => ({
    animationDelay,
    animationDuration,
    backgroundColor,
    height,
  }),
});
