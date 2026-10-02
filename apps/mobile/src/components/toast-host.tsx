import { useSyncExternalStore } from "react";
import { Pressable, StyleSheet, Text } from "react-native";
import Animated, { FadeInUp, FadeOutUp } from "react-native-reanimated";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { CornerCurve, Radius, Spacing, Typography } from "@/constants/theme";
import { dismissToast, getToast, subscribeToast } from "@/lib/toast";
import { createStyleHook } from "@/settings/theme-provider";

export function ToastHost() {
  const styles = useStyles();
  const insets = useSafeAreaInsets();
  const toast = useSyncExternalStore(subscribeToast, getToast, getToast);
  if (!toast) return null;

  return (
    <Animated.View
      key={toast.id}
      entering={FadeInUp}
      exiting={FadeOutUp}
      pointerEvents="box-none"
      style={[styles.container, { top: insets.top + Spacing.sm }]}
    >
      <Pressable
        accessibilityHint="Dismisses the notification"
        accessibilityLiveRegion="polite"
        accessibilityRole="alert"
        onPress={() => dismissToast(toast.id)}
        style={styles.toast}
      >
        <Text style={styles.title}>{toast.title}</Text>
        {toast.description && (
          <Text style={styles.description}>{toast.description}</Text>
        )}
      </Pressable>
    </Animated.View>
  );
}

const useStyles = createStyleHook((Colors) => ({
  container: {
    position: "absolute",
    left: Spacing.md,
    right: Spacing.md,
    zIndex: 1000,
  },
  toast: {
    gap: Spacing.xs,
    padding: Spacing.md,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.alertBorder,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.alert,
    shadowColor: Colors.ink,
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.1,
    shadowRadius: 8,
    elevation: 4,
  },
  title: {
    ...Typography.label,
    color: Colors.alertForeground,
  },
  description: {
    ...Typography.caption,
    color: Colors.alertForeground,
  },
}));
