import { useSyncExternalStore } from "react";
import { ActivityIndicator, StyleSheet, Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { SettingsContent } from "@/components/profile-sheet";
import { Colors, Spacing, Typography } from "@/constants/theme";
import { getMobileSyncSnapshot, subscribeMobileSync } from "@/sync/mobile-sync";

export function SyncEnrollmentScreen() {
  const sync = useSyncExternalStore(
    subscribeMobileSync,
    getMobileSyncSnapshot,
    getMobileSyncSnapshot,
  );
  const loading =
    sync.phase === "inactive" ||
    (sync.phase === "starting" && !sync.hasRecoveryKey);

  if (loading) {
    return (
      <View style={styles.loading}>
        <ActivityIndicator color={Colors.ink} />
        <Text style={styles.loadingText}>Checking encrypted sync…</Text>
      </View>
    );
  }

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.intro}>
        <Text style={styles.title}>Connect encrypted sync</Text>
        <Text style={styles.description}>
          Mobile capture is a Pro sync companion. Set up your recovery key once,
          then you can keep recording when this phone is offline.
        </Text>
      </View>
      <SettingsContent
        initialMode={sync.phase === "setup_required" ? "choose" : "status"}
      />
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeArea: {
    flex: 1,
    backgroundColor: Colors.background,
  },
  loading: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.sm,
    backgroundColor: Colors.background,
  },
  loadingText: {
    ...Typography.caption,
    color: Colors.muted,
  },
  intro: {
    gap: Spacing.sm,
    paddingHorizontal: Spacing.lg,
    paddingTop: Spacing.md,
    paddingBottom: Spacing.lg,
  },
  title: {
    ...Typography.title,
    color: Colors.ink,
  },
  description: {
    ...Typography.body,
    color: Colors.muted,
  },
});
