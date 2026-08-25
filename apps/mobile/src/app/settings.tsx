import { useRouter } from "expo-router";
import { StyleSheet, Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { SettingsContent } from "@/components/profile-sheet";
import { IconButton } from "@/components/ui/icon-button";
import { Colors, ControlSize, Spacing, Typography } from "@/constants/theme";

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
          icon="arrow-back"
          iconSize={22}
          onPress={handleBack}
        />
        <Text style={styles.title}>Settings</Text>
        <View style={styles.headerSpacer} />
      </View>
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
    width: ControlSize.compact,
    height: ControlSize.compact,
  },
});
