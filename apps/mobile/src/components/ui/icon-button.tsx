import { Pressable, StyleSheet } from "react-native";

import { ControlSize, CornerCurve, Radius } from "@/constants/theme";
import { createStyleHook, useColors } from "@/settings/theme-provider";

import { NativeIcon, type NativeIconName } from "./native-icon";

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
  icon: NativeIconName;
  iconSize?: number;
  onPress: () => void;
  tone?: "default" | "muted" | "destructive";
  variant?: "ghost" | "surface";
}) {
  const styles = useStyles();
  const Colors = useColors();
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
      onPress={onPress}
      style={({ pressed }) => [
        styles.button,
        variant === "surface" && styles.surface,
        pressed && styles.pressed,
        disabled && styles.disabled,
      ]}
    >
      <NativeIcon name={icon} size={iconSize} color={color} />
    </Pressable>
  );
}

const useStyles = createStyleHook((Colors) => ({
  button: {
    width: ControlSize.default,
    height: ControlSize.default,
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
}));
