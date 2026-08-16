import { Ionicons } from "@expo/vector-icons";
import type { ComponentProps } from "react";
import { Pressable, StyleSheet } from "react-native";

import { Colors, ControlSize, CornerCurve, Radius } from "@/constants/theme";

export function IconButton({
  accessibilityLabel,
  disabled = false,
  icon,
  iconSize = 20,
  onPress,
  tone = "default",
  variant = "ghost",
}: {
  accessibilityLabel: string;
  disabled?: boolean;
  icon: ComponentProps<typeof Ionicons>["name"];
  iconSize?: number;
  onPress: () => void;
  tone?: "default" | "muted" | "destructive";
  variant?: "ghost" | "surface";
}) {
  const color =
    tone === "destructive"
      ? Colors.destructive
      : tone === "muted"
        ? Colors.muted
        : Colors.ink;

  return (
    <Pressable
      accessibilityLabel={accessibilityLabel}
      accessibilityRole="button"
      disabled={disabled}
      hitSlop={4}
      onPress={onPress}
      style={({ pressed }) => [
        styles.button,
        variant === "surface" && styles.surface,
        pressed && styles.pressed,
        disabled && styles.disabled,
      ]}
    >
      <Ionicons name={icon} size={iconSize} color={color} />
    </Pressable>
  );
}

const styles = StyleSheet.create({
  button: {
    width: ControlSize.compact,
    height: ControlSize.compact,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
  },
  surface: {
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    backgroundColor: Colors.surface,
  },
  pressed: {
    backgroundColor: Colors.accentSurface,
  },
  disabled: {
    opacity: 0.45,
  },
});
