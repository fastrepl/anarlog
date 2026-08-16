import { memo, useMemo } from "react";
import { StyleSheet, View } from "react-native";
import Animated, {
  cancelAnimation,
  Easing,
  useAnimatedStyle,
  useSharedValue,
  withDelay,
  withRepeat,
  withSequence,
  withTiming,
} from "react-native-reanimated";

import { CornerCurve } from "@/constants/theme";
import { useMountEffect } from "@/lib/use-mount-effect";

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

function generatePattern(count: number): number[] {
  if (count <= 1) return [100];

  const mid = (count - 1) / 2;
  return Array.from({ length: count }, (_, index) => {
    const distance = Math.abs(index - mid) / mid;
    return 50 + 50 * (1 - distance);
  });
}

function DancingStick({
  color,
  delayMs,
  durationMs,
  height,
  maxScaleY,
  width,
}: {
  color: string;
  delayMs: number;
  durationMs: number;
  height: number;
  maxScaleY: number;
  width: number;
}) {
  const scale = useSharedValue(0.2);

  useMountEffect(() => {
    const halfDuration = durationMs / 2;
    scale.value = withDelay(
      delayMs,
      withRepeat(
        withSequence(
          withTiming(1, {
            duration: halfDuration,
            easing: Easing.inOut(Easing.ease),
          }),
          withTiming(0.2, {
            duration: halfDuration,
            easing: Easing.inOut(Easing.ease),
          }),
        ),
        -1,
        false,
      ),
    );
    return () => cancelAnimation(scale);
  });

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scaleY: scale.value }],
  }));

  return (
    <View
      style={{
        width,
        height,
        transform: [{ scaleY: maxScaleY }],
      }}
    >
      <Animated.View
        style={[styles.stick, { backgroundColor: color }, animatedStyle]}
      />
    </View>
  );
}

export const DancingSticks = memo(function DancingSticks({
  amplitude,
  color,
  gap,
  height,
  stickWidth,
  width,
}: {
  amplitude: number;
  color: string;
  gap: number;
  height: number;
  stickWidth: number;
  width: number;
}) {
  const stickCount = Math.max(
    1,
    Math.floor((width + gap) / (stickWidth + gap)),
  );
  const pattern = useMemo(() => generatePattern(stickCount), [stickCount]);
  const sticks = useMemo(
    () =>
      pattern.map((baseLength, index) => {
        const maxScaleY = Math.max(0.25, Math.min(1, baseLength / 100));
        const rng = mulberry32((index + 1) * 10007);
        const speed = 4 + rng() * 3;
        const durationMs = (Math.PI * 2 * 1000) / speed;
        const delayMs = (rng() * Math.PI * 2 * 1000) / speed;
        return { delayMs, durationMs, maxScaleY };
      }),
    [pattern],
  );
  const amplitudeScale = 0.2 + 0.8 * Math.max(0, Math.min(1, amplitude));
  const isFlat = amplitude === 0;

  return (
    <View style={[styles.root, { height, width }]}>
      <View
        style={{
          width,
          height: 1,
          backgroundColor: color,
          opacity: isFlat ? 1 : 0,
        }}
      />
      <View
        style={[
          styles.container,
          {
            gap,
            height,
            width,
            opacity: isFlat ? 0 : 1,
            transform: [{ scaleY: amplitudeScale }],
          },
        ]}
      >
        {sticks.map(({ delayMs, durationMs, maxScaleY }, index) => (
          <DancingStick
            key={index}
            color={color}
            delayMs={delayMs}
            durationMs={durationMs}
            height={height}
            maxScaleY={maxScaleY}
            width={stickWidth}
          />
        ))}
      </View>
    </View>
  );
});

const styles = StyleSheet.create({
  root: {
    alignItems: "center",
    justifyContent: "center",
  },
  container: {
    position: "absolute",
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
  },
  stick: {
    width: "100%",
    height: "100%",
    borderRadius: 999,
    borderCurve: CornerCurve.squircle,
  },
});
