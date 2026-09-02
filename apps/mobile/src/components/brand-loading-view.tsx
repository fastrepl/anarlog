import { Image } from "expo-image";
import { StyleSheet, View } from "react-native";
import Animated, {
  cancelAnimation,
  Easing,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withDelay,
  withSequence,
  withTiming,
} from "react-native-reanimated";
import { scheduleOnRN } from "react-native-worklets";

import { Colors } from "@/constants/theme";
import { useMountEffect } from "@/lib/use-mount-effect";

const WORDMARK_WIDTH = 200;
const WORDMARK_HEIGHT = 56;
const WORDMARK_SOURCE_WIDTH = 1_205;
const REVEAL_STEPS = [154, 357, 538, 714, 786, 1_006, 1_205].map(
  (width) => (width / WORDMARK_SOURCE_WIDTH) * WORDMARK_WIDTH,
);
const FIRST_REVEAL = REVEAL_STEPS[0];
const LOGO_SHIFT = -(WORDMARK_WIDTH - FIRST_REVEAL) / 2;
const EASE_OUT = Easing.bezier(0.23, 1, 0.32, 1);

export function BrandLoadingView({
  animated = false,
  onAnimationComplete,
}: {
  animated?: boolean;
  onAnimationComplete?: () => void;
}) {
  const reducedMotion = useReducedMotion();
  const shouldAnimate = animated && !reducedMotion;
  const logoOpacity = useSharedValue(shouldAnimate ? 1 : 0);
  const logoScale = useSharedValue(1);
  const logoTranslateX = useSharedValue(0);
  const revealWidth = useSharedValue(
    shouldAnimate ? FIRST_REVEAL : WORDMARK_WIDTH,
  );
  const wordmarkOpacity = useSharedValue(shouldAnimate ? 0 : 1);
  const cursorOpacity = useSharedValue(0);

  useMountEffect(() => {
    if (!animated || reducedMotion) {
      onAnimationComplete?.();
      return;
    }

    logoScale.set(
      withDelay(160, withTiming(0.6, { duration: 200, easing: EASE_OUT })),
    );
    logoTranslateX.set(
      withDelay(
        160,
        withTiming(LOGO_SHIFT, { duration: 200, easing: EASE_OUT }),
      ),
    );
    logoOpacity.set(
      withDelay(320, withTiming(0, { duration: 100, easing: EASE_OUT })),
    );
    wordmarkOpacity.set(
      withDelay(400, withTiming(1, { duration: 100, easing: EASE_OUT })),
    );
    revealWidth.set(
      withSequence(
        ...REVEAL_STEPS.slice(1).map((width, index) =>
          withDelay(index === 0 ? 560 : 90, withTiming(width, { duration: 1 })),
        ),
      ),
    );
    cursorOpacity.set(
      withSequence(
        withDelay(400, withTiming(1, { duration: 100, easing: EASE_OUT })),
        withDelay(650, withTiming(1, { duration: 1 })),
        withTiming(0, { duration: 100 }),
        withTiming(1, { duration: 100 }),
        withTiming(0, { duration: 100 }, (finished) => {
          if (finished && onAnimationComplete) {
            scheduleOnRN(onAnimationComplete);
          }
        }),
      ),
    );

    return () => {
      cancelAnimation(cursorOpacity);
      cancelAnimation(logoOpacity);
      cancelAnimation(logoScale);
      cancelAnimation(logoTranslateX);
      cancelAnimation(revealWidth);
      cancelAnimation(wordmarkOpacity);
    };
  });

  const logoStyle = useAnimatedStyle(() => ({
    opacity: logoOpacity.get(),
    transform: [
      { translateX: logoTranslateX.get() },
      { scale: logoScale.get() },
    ],
  }));
  const wordmarkStyle = useAnimatedStyle(() => ({
    opacity: wordmarkOpacity.get(),
  }));
  const coverStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: revealWidth.get() }],
  }));
  const cursorStyle = useAnimatedStyle(() => ({
    opacity: cursorOpacity.get(),
    transform: [{ translateX: revealWidth.get() + 4 }],
  }));

  return (
    <View
      accessible
      accessibilityLabel="Loading Anarlog"
      accessibilityRole="progressbar"
      style={styles.container}
    >
      <View style={styles.animationFrame}>
        <Animated.View style={[styles.mark, logoStyle]}>
          <Image
            contentFit="contain"
            source={require("../../assets/images/splash-icon.png")}
            style={StyleSheet.absoluteFill}
          />
        </Animated.View>
        <Animated.View style={[styles.wordmark, wordmarkStyle]}>
          <View style={styles.wordmarkClip}>
            <Image
              contentFit="contain"
              source={require("../../assets/images/anarlog-wordmark.png")}
              style={StyleSheet.absoluteFill}
            />
            <Animated.View style={[styles.wordmarkCover, coverStyle]} />
          </View>
          <Animated.View style={[styles.cursor, cursorStyle]} />
        </Animated.View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: Colors.brandBackgroundTop,
  },
  animationFrame: {
    width: WORDMARK_WIDTH,
    height: WORDMARK_HEIGHT,
    alignItems: "center",
    justifyContent: "center",
  },
  mark: {
    position: "absolute",
    width: 76,
    height: 76,
  },
  wordmark: {
    position: "absolute",
    width: WORDMARK_WIDTH,
    height: WORDMARK_HEIGHT,
  },
  wordmarkClip: {
    width: WORDMARK_WIDTH,
    height: WORDMARK_HEIGHT,
    overflow: "hidden",
  },
  wordmarkCover: {
    ...StyleSheet.absoluteFill,
    backgroundColor: Colors.brandBackgroundTop,
  },
  cursor: {
    position: "absolute",
    top: 44,
    left: 0,
    width: 14,
    height: 5,
    borderRadius: 2.5,
    backgroundColor: Colors.ink,
  },
});
