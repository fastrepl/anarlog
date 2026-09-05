import { useMutation } from "@tanstack/react-query";
import * as LocalAuthentication from "expo-local-authentication";
import { useSyncExternalStore } from "react";
import { AppState, Modal, Text, View } from "react-native";

import { Button } from "@/components/ui/button";
import { Spacing, Typography } from "@/constants/theme";
import { useMountEffect } from "@/lib/use-mount-effect";
import { createStyleHook } from "@/settings/theme-provider";

import { AppLockController } from "./app-lock-model";
import { loadPrivacyPreferences, usePrivacyPreferences } from "./privacy-store";

const controller = new AppLockController();

export async function authenticateForAppLock(): Promise<boolean> {
  if (!controller.beginAuthentication()) return false;
  let success = false;
  try {
    const result = await LocalAuthentication.authenticateAsync({
      promptMessage: "Unlock Anarlog",
      cancelLabel: "Cancel",
      fallbackLabel: "Use passcode",
      biometricsSecurityLevel: "strong",
    });
    success = result.success;
    return success;
  } finally {
    controller.finishAuthentication(success);
  }
}

export function AppLock() {
  const styles = useStyles();
  const preferences = usePrivacyPreferences();
  const { locked, covered } = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  const load = useMutation({ mutationFn: loadPrivacyPreferences });
  const unlock = useMutation({ mutationFn: authenticateForAppLock });
  useMountEffect(() => {
    load.mutate();
    controller.appStateChanged(AppState.currentState);
    const subscription = AppState.addEventListener("change", (state) =>
      controller.appStateChanged(state),
    );
    return () => subscription.remove();
  });
  if (preferences.ready && (!preferences.appLock || (!locked && !covered)))
    return null;
  return (
    <Modal visible animationType="none" onRequestClose={() => {}}>
      <View style={styles.cover}>
        <Text style={styles.title}>Anarlog</Text>
        {preferences.ready ? (
          <Button
            label="Unlock"
            loading={unlock.isPending}
            onPress={() => unlock.mutate()}
          />
        ) : load.error ? (
          <Button label="Try again" onPress={() => load.mutate()} />
        ) : (
          <Text style={styles.copy}>Opening your notes…</Text>
        )}
        {unlock.error && (
          <Text style={styles.copy}>Could not unlock. Please try again.</Text>
        )}
      </View>
    </Modal>
  );
}

const useStyles = createStyleHook((Colors) => ({
  cover: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
    gap: Spacing.lg,
    backgroundColor: Colors.background,
  },
  title: { ...Typography.title, color: Colors.ink },
  copy: { ...Typography.body, color: Colors.muted },
}));
