import { useEffect } from "react";
import { StyleSheet, View } from "react-native";
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withDelay,
  withRepeat,
  withSequence,
  withTiming,
} from "react-native-reanimated";

import { Colors, CornerCurve } from "@/constants/theme";

export const WAVEFORM_BAR_COUNT = 28;

function IdleBar({ index }: { index: number }) {
  const peak = 0.35 + 0.65 * Math.abs(Math.sin(index * 2.7));
  const scale = useSharedValue(0.2);

  useEffect(() => {
    scale.value = withDelay(
      index * 40,
      withRepeat(
        withSequence(
          withTiming(peak, { duration: 320 }),
          withTiming(0.2, { duration: 320 }),
        ),
        -1,
        true,
      ),
    );
  }, [index, peak, scale]);

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scaleY: scale.value }],
  }));

  return <Animated.View style={[styles.bar, animatedStyle]} />;
}

export function Waveform({ levels }: { levels: number[] | null }) {
  if (!levels) {
    return (
      <View style={styles.container}>
        {Array.from({ length: WAVEFORM_BAR_COUNT }, (_, index) => (
          <IdleBar key={index} index={index} />
        ))}
      </View>
    );
  }

  return (
    <View style={styles.container}>
      {levels.map((level, index) => (
        <View
          key={index}
          style={[styles.bar, { transform: [{ scaleY: 0.1 + 0.9 * level }] }]}
        />
      ))}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-evenly",
    height: 72,
  },
  bar: {
    width: 3,
    height: 56,
    borderRadius: 1.5,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.inkInverse,
  },
});
