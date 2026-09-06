import type { IconName } from "@expo/ui";
import { FieldGroup, Host, Icon, ListItem, Text as NativeText } from "@expo/ui";
import {
  font,
  listSectionSpacing,
  scrollContentBackground,
} from "@expo/ui/swift-ui/modifiers";
import { useRouter } from "expo-router";
import type { ReactNode } from "react";
import { Platform, ScrollView, Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { IconButton } from "@/components/ui/icon-button";
import { ControlSize, Spacing, Typography } from "@/constants/theme";
import {
  createStyleHook,
  useAppColorScheme,
  useColors,
} from "@/settings/theme-provider";

export function SettingsPage({
  title,
  children,
  layout = "form",
}: {
  title: string;
  children: ReactNode;
  layout?: "form" | "menu";
}) {
  const styles = useStyles();
  const Colors = useColors();
  const router = useRouter();
  const colorScheme = useAppColorScheme();
  return (
    <SafeAreaView style={styles.page}>
      <View style={styles.header}>
        <IconButton
          accessibilityLabel="Back"
          icon="back"
          iconSize={22}
          onPress={() =>
            router.canGoBack() ? router.back() : router.replace("/")
          }
        />
        <Text style={styles.title}>{title}</Text>
        <View style={styles.spacer} />
      </View>
      {layout === "menu" ? (
        <ScrollView contentContainerStyle={styles.menu}>{children}</ScrollView>
      ) : (
        <Host
          style={styles.form}
          colorScheme={colorScheme}
          seedColor={colorScheme === "dark" ? Colors.muted : Colors.ink}
          ignoreSafeArea="all"
        >
          <FieldGroup
            style={{ backgroundColor: Colors.background }}
            modifiers={
              Platform.OS === "ios"
                ? [
                    scrollContentBackground("hidden"),
                    listSectionSpacing(Spacing.lg),
                    font({ textStyle: "subheadline" }),
                  ]
                : undefined
            }
          >
            {children}
          </FieldGroup>
        </Host>
      )}
    </SafeAreaView>
  );
}

export function SettingsRow({
  title,
  value,
  description,
  icon,
  onPress,
}: {
  title: string;
  value?: string;
  description?: string;
  icon?: IconName;
  onPress?: () => void;
}) {
  return (
    <ListItem
      onPress={onPress}
      leading={icon ? <Icon name={icon} size={20} /> : undefined}
      supportingText={description ?? (onPress ? value : undefined)}
      trailing={
        onPress ? (
          <Icon
            name={Icon.select({
              ios: "chevron.right",
              android: import("@expo/material-symbols/chevron_right.xml"),
            })}
            size={14}
          />
        ) : value ? (
          <NativeText>{value}</NativeText>
        ) : undefined
      }
    >
      <NativeText>{title}</NativeText>
    </ListItem>
  );
}

export function SettingsError({ error }: { error: unknown }) {
  if (!error) return null;
  return (
    <FieldGroup.SectionFooter>
      <NativeText>Could not complete this change. Please try again.</NativeText>
    </FieldGroup.SectionFooter>
  );
}

const useStyles = createStyleHook((Colors) => ({
  page: { flex: 1, backgroundColor: Colors.background },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: Spacing.md,
    paddingTop: Spacing.sm,
    paddingBottom: Spacing.md,
  },
  title: { ...Typography.section, color: Colors.ink },
  spacer: { width: ControlSize.default, height: ControlSize.default },
  form: { flex: 1 },
  menu: {
    paddingHorizontal: Spacing.md,
    paddingTop: Spacing.md,
    paddingBottom: Spacing.lg,
    gap: Spacing.lg,
  },
}));
