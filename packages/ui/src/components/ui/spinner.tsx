import * as stylex from "@stylexjs/stylex";
import React from "react";

import { radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

interface SpinnerProps
  extends React.HTMLAttributes<HTMLDivElement>, StyleXProps {
  size?: number;
  color?: string;
}

const Spinner = React.forwardRef<HTMLDivElement, SpinnerProps>(
  (
    { size = 16, color = "currentColor", className, style, sx, ...props },
    ref,
  ) => {
    const segments = Array.from({ length: 12 });

    return (
      <div
        {...props}
        {...mergeStyleXProps(
          [styles.root, styles.rootSize(size, color), sx],
          className,
          style,
        )}
        ref={ref}
      >
        {segments.map((_, i) => {
          const rotation = i * 30;
          const animationDelay = `${i * (1 / 12)}s`;

          return (
            <div
              key={i}
              {...stylex.props(
                styles.segment,
                styles.segmentRotation(`rotate(${rotation}deg)`),
              )}
            >
              <div
                {...stylex.props(
                  styles.indicator,
                  styles.indicatorSize(size * 0.1, size * 0.26, animationDelay),
                )}
              />
            </div>
          );
        })}
      </div>
    );
  },
);

Spinner.displayName = "Spinner";

const opacitySpin = stylex.keyframes({
  "0%": {
    opacity: 1,
  },
  "8.33%": {
    opacity: 0.9,
  },
  "16.67%": {
    opacity: 0.8,
  },
  "25%": {
    opacity: 0.7,
  },
  "33.33%": {
    opacity: 0.6,
  },
  "41.67%": {
    opacity: 0.5,
  },
  "50%": {
    opacity: 0.4,
  },
  "58.33%": {
    opacity: 0.35,
  },
  "66.67%": {
    opacity: 0.3,
  },
  "75%": {
    opacity: 0.25,
  },
  "83.33%": {
    opacity: 0.2,
  },
  "91.67%": {
    opacity: 0.15,
  },
  "100%": {
    opacity: 1,
  },
});

const styles = stylex.create({
  root: {
    display: "inline-block",
    position: "relative",
  },
  rootSize: (size: number, color: string) => ({
    color,
    height: size,
    width: size,
  }),
  segment: {
    height: "100%",
    left: 0,
    position: "absolute",
    top: 0,
    width: "100%",
  },
  segmentRotation: (transform: string) => ({
    transform,
  }),
  indicator: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: opacitySpin,
    animationTimingFunction: "linear",
    backgroundColor: "currentColor",
    borderRadius: radii.full,
    left: "50%",
    position: "absolute",
    top: 0,
    transform: "translateX(-50%)",
  },
  indicatorSize: (width: number, height: number, animationDelay: string) => ({
    animationDelay,
    height,
    width,
  }),
});

export { Spinner };
