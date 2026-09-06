import { Host, Icon, type IconName } from "@expo/ui";
import { Children, type ReactNode } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { Card } from "@/components/ui/card";
import { Spacing, Typography } from "@/constants/theme";
import { createStyleHook, useColors } from "@/settings/theme-provider";

export function SettingsMenuGroup({ children }: { children: ReactNode }) {
  const styles = useStyles();
  return (
    <Card style={styles.card}>
      {Children.toArray(children).map((child, index) => (
        <View key={index}>
          {index > 0 && <View style={styles.separator} />}
          {child}
        </View>
      ))}
    </Card>
  );
}

export function SettingsMenuLink({
  title,
  icon,
  onPress,
}: {
  title: string;
  icon: IconName;
  onPress: () => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={title}
      onPress={onPress}
      style={({ pressed }) => [styles.row, pressed && styles.pressed]}
    >
      <Host style={styles.icon} pointerEvents="none">
        <Icon name={icon} size={20} color={Colors.muted} />
      </Host>
      <Text style={styles.title}>{title}</Text>
      <Host style={styles.chevron} pointerEvents="none">
        <Icon
          name={Icon.select({
            ios: "chevron.right",
            android: import("@expo/material-symbols/chevron_right.xml"),
          })}
          size={12}
          color={Colors.muted}
        />
      </Host>
    </Pressable>
  );
}

const useStyles = createStyleHook((Colors) => ({
  card: { overflow: "hidden" },
  row: {
    flexDirection: "row",
    alignItems: "center",
    minHeight: 64,
    paddingHorizontal: Spacing.compact,
    paddingVertical: Spacing.compact,
    gap: Spacing.compact,
  },
  pressed: { backgroundColor: Colors.accentSurface },
  icon: { width: 24, height: 24 },
  chevron: { width: 16, height: 20 },
  title: { ...Typography.bodyStrong, flex: 1, color: Colors.ink },
  separator: {
    height: StyleSheet.hairlineWidth,
    backgroundColor: Colors.border,
    marginLeft: Spacing.compact * 2 + 24,
    marginRight: Spacing.compact,
  },
}));
