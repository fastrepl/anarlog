import type { ReactNode } from "react";
import { StyleSheet, View, type StyleProp, type ViewStyle } from "react-native";

import { Colors, CornerCurve, Radius } from "@/constants/theme";

export function Card({
  children,
  style,
  tone = "surface",
}: {
  children: ReactNode;
  style?: StyleProp<ViewStyle>;
  tone?: "surface" | "muted" | "alert";
}) {
  return (
    <View
      style={[
        styles.card,
        tone === "muted"
          ? styles.muted
          : tone === "alert"
            ? styles.alert
            : styles.surface,
        style,
      ]}
    >
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
  },
  surface: {
    borderColor: Colors.border,
    backgroundColor: Colors.surface,
  },
  muted: {
    borderColor: Colors.border,
    backgroundColor: Colors.mutedSurface,
  },
  alert: {
    borderColor: Colors.alertBorder,
    backgroundColor: Colors.alert,
  },
});
