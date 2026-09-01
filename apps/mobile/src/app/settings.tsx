import { Ionicons } from "@expo/vector-icons";
import { useRouter } from "expo-router";
import { Platform, Pressable, StyleSheet, Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { SettingsContent } from "@/components/profile-sheet";
import { IconButton } from "@/components/ui/icon-button";
import {
  Colors,
  ControlSize,
  CornerCurve,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";

export default function SettingsScreen() {
  const router = useRouter();

  const handleBack = () => {
    if (router.canGoBack()) router.back();
    else router.replace("/");
  };

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        <IconButton
          accessibilityLabel="Back"
          icon="back"
          iconSize={22}
          onPress={handleBack}
        />
        <Text style={styles.title}>Settings</Text>
        <View style={styles.headerSpacer} />
      </View>
      {Platform.OS === "ios" && (
        <Pressable
          accessibilityRole="button"
          onPress={() => router.push("/action-button")}
          style={({ pressed }) => [
            styles.actionButtonRow,
            pressed && styles.actionButtonRowPressed,
          ]}
        >
          <View style={styles.actionButtonIcon}>
            <Ionicons name="mic-outline" size={20} color={Colors.ink} />
          </View>
          <View style={styles.actionButtonCopy}>
            <Text style={styles.actionButtonTitle}>Action Button</Text>
            <Text style={styles.actionButtonDescription}>
              Start or stop listening in one press
            </Text>
          </View>
          <Ionicons name="chevron-forward" size={18} color={Colors.muted} />
        </Pressable>
      )}
      <SettingsContent />
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeArea: {
    flex: 1,
    backgroundColor: Colors.background,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: Spacing.lg,
    paddingVertical: Spacing.md,
  },
  title: {
    ...Typography.section,
    color: Colors.ink,
  },
  headerSpacer: {
    width: ControlSize.default,
    height: ControlSize.default,
  },
  actionButtonRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
    marginHorizontal: Spacing.lg,
    marginBottom: Spacing.lg,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.surface,
    padding: Spacing.md,
  },
  actionButtonRowPressed: {
    opacity: 0.78,
  },
  actionButtonIcon: {
    width: 40,
    height: 40,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accentSurface,
  },
  actionButtonCopy: {
    flex: 1,
  },
  actionButtonTitle: {
    ...Typography.bodyStrong,
    color: Colors.ink,
  },
  actionButtonDescription: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
});
