import type { ReactNode } from "react";
import {
  ActivityIndicator,
  Pressable,
  StyleSheet,
  Text,
  type StyleProp,
  type ViewStyle,
} from "react-native";

import {
  Colors,
  ControlSize,
  CornerCurve,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";

export function Button({
  label,
  leading,
  loading = false,
  disabled = false,
  onPress,
  size = "default",
  style,
  variant = "primary",
}: {
  label: string;
  leading?: ReactNode;
  loading?: boolean;
  disabled?: boolean;
  onPress: () => void;
  size?: "small" | "default" | "large";
  style?: StyleProp<ViewStyle>;
  variant?: "primary" | "secondary" | "outline" | "ghost" | "destructive";
}) {
  const foreground =
    variant === "primary"
      ? Colors.primaryForeground
      : variant === "destructive"
        ? Colors.destructiveForeground
        : Colors.ink;
  const variantStyle =
    variant === "primary"
      ? styles.primary
      : variant === "secondary"
        ? styles.secondary
        : variant === "outline"
          ? styles.outline
          : variant === "destructive"
            ? styles.destructive
            : styles.ghost;
  const sizeStyle =
    size === "small"
      ? styles.small
      : size === "large"
        ? styles.large
        : styles.default;

  return (
    <Pressable
      accessibilityRole="button"
      disabled={disabled || loading}
      onPress={onPress}
      style={({ pressed }) => [
        styles.base,
        variantStyle,
        sizeStyle,
        pressed && styles.pressed,
        (disabled || loading) && styles.disabled,
        style,
      ]}
    >
      {loading ? (
        <ActivityIndicator size="small" color={foreground} />
      ) : (
        <>
          {leading}
          <Text style={[styles.label, { color: foreground }]}>{label}</Text>
        </>
      )}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  base: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.sm,
    paddingHorizontal: Spacing.md,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
  },
  primary: {
    backgroundColor: Colors.primary,
  },
  secondary: {
    backgroundColor: Colors.secondary,
  },
  outline: {
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    backgroundColor: Colors.surface,
  },
  ghost: {
    backgroundColor: "transparent",
  },
  destructive: {
    backgroundColor: Colors.destructive,
  },
  small: {
    height: ControlSize.compact,
    paddingHorizontal: Spacing.compact,
  },
  default: {
    height: ControlSize.default,
  },
  large: {
    height: ControlSize.large,
    paddingHorizontal: Spacing.lg,
  },
  pressed: {
    opacity: 0.78,
  },
  disabled: {
    opacity: 0.5,
  },
  label: {
    ...Typography.label,
  },
});
