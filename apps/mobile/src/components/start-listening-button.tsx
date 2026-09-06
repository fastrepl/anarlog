import { Pressable, Text, View } from "react-native";

import {
  CornerCurve,
  LISTENING_CONTROL_HEIGHT,
  LISTENING_CONTROL_RADIUS,
  Spacing,
  Typography,
} from "@/constants/theme";
import { createStyleHook } from "@/settings/theme-provider";

export function StartListeningButton({
  bottomSpacing = Spacing.md,
  onPress,
}: {
  bottomSpacing?: number;
  onPress: () => void;
}) {
  const styles = useStyles();
  return (
    <Pressable
      accessibilityRole="button"
      onPress={onPress}
      style={({ pressed }) => [
        styles.button,
        { marginBottom: bottomSpacing },
        pressed && styles.buttonPressed,
      ]}
    >
      <View style={styles.dot} />
      <Text style={styles.label}>Start listening</Text>
    </Pressable>
  );
}

const useStyles = createStyleHook((Colors) => ({
  button: {
    height: LISTENING_CONTROL_HEIGHT,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.sm,
    marginHorizontal: Spacing.md,
    borderRadius: LISTENING_CONTROL_RADIUS,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.primary,
  },
  buttonPressed: {
    opacity: 0.6,
  },
  dot: {
    width: 12,
    height: 12,
    borderRadius: 6,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accent,
  },
  label: {
    ...Typography.section,
    color: Colors.primaryForeground,
  },
}));
