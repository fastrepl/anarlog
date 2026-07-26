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

import { Colors } from "@/constants/theme";

const BAR_COUNT = 28;

function Bar({ index }: { index: number }) {
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

export function Waveform() {
  return (
    <View style={styles.container}>
      {Array.from({ length: BAR_COUNT }, (_, index) => (
        <Bar key={index} index={index} />
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
    backgroundColor: Colors.inkInverse,
  },
});
