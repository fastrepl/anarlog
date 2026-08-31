import { useMemo } from "react";
import { StyleSheet, Text, useWindowDimensions, View } from "react-native";
import { Gesture, GestureDetector } from "react-native-gesture-handler";
import Animated, {
  Easing,
  Extrapolation,
  interpolate,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withSpring,
  withTiming,
} from "react-native-reanimated";
import { scheduleOnRN } from "react-native-worklets";

import { IPhoneDeviceFrame } from "@/components/iphone-device-frame";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Colors, Spacing, Typography } from "@/constants/theme";

const EASE_OUT = Easing.bezier(0.23, 1, 0.32, 1);

function project(velocity: number, decelerationRate = 0.998) {
  "worklet";
  return ((velocity / 1_000) * decelerationRate) / (1 - decelerationRate);
}

export function ActionButtonCard({
  onConfigure,
  onDismiss,
}: {
  onConfigure: () => void;
  onDismiss: () => void;
}) {
  const { width } = useWindowDimensions();
  const reducedMotion = useReducedMotion();
  const translateX = useSharedValue(0);
  const gestureStartX = useSharedValue(0);
  const dismissThreshold = width * 0.28;

  const pan = useMemo(
    () =>
      Gesture.Pan()
        .activeOffsetX([-10, 10])
        .failOffsetY([-10, 10])
        .onStart(() => {
          gestureStartX.set(translateX.get());
        })
        .onUpdate((event) => {
          translateX.set(gestureStartX.get() + event.translationX);
        })
        .onEnd((event) => {
          const projectedX = translateX.get() + project(event.velocityX);
          if (Math.abs(projectedX) > dismissThreshold) {
            const direction = projectedX < 0 ? -1 : 1;
            translateX.set(
              withTiming(
                direction * width,
                { duration: 200, easing: EASE_OUT },
                (finished) => {
                  if (finished) scheduleOnRN(onDismiss);
                },
              ),
            );
            return;
          }
          translateX.set(
            withSpring(0, {
              duration: 400,
              dampingRatio: 1,
              velocity: event.velocityX,
            }),
          );
        }),
    [dismissThreshold, gestureStartX, onDismiss, translateX, width],
  );

  const animatedStyle = useAnimatedStyle(() => {
    const offset = translateX.get();
    return {
      opacity: interpolate(
        Math.abs(offset),
        [0, width * 0.65],
        [1, 0],
        Extrapolation.CLAMP,
      ),
      transform: [{ translateX: reducedMotion ? 0 : offset }],
    };
  });

  return (
    <GestureDetector gesture={pan}>
      <Animated.View style={animatedStyle}>
        <Card style={styles.card}>
          <View
            accessible
            accessibilityActions={[{ name: "dismiss", label: "Dismiss" }]}
            accessibilityLabel="Listen with the Action Button"
            onAccessibilityAction={(event) => {
              if (event.nativeEvent.actionName === "dismiss") onDismiss();
            }}
            style={styles.heading}
          >
            <IPhoneDeviceFrame width={30} />
            <Text style={styles.title}>Listen with the Action Button</Text>
          </View>
          <Text style={styles.description}>
            Long-press once to start a meeting recording and again to stop.
          </Text>
          <Button label="Configure Action Button" onPress={onConfigure} />
        </Card>
      </Animated.View>
    </GestureDetector>
  );
}

const styles = StyleSheet.create({
  card: {
    marginBottom: Spacing.lg,
    padding: Spacing.md,
    gap: Spacing.md,
  },
  heading: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.md,
  },
  title: {
    flex: 1,
    ...Typography.section,
    color: Colors.ink,
  },
  description: {
    ...Typography.body,
    color: Colors.muted,
  },
});
