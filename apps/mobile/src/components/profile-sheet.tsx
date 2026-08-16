import { Modal, Pressable, StyleSheet, Text, View } from "react-native";

import { useAuth } from "@/auth/context";
import { Button } from "@/components/ui/button";
import {
  Colors,
  CornerCurve,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";

export function ProfileSheet({
  visible,
  onClose,
}: {
  visible: boolean;
  onClose: () => void;
}) {
  const auth = useAuth();
  const planLabel = auth.bypass
    ? "Local dev"
    : auth.billing.plan === "trial"
      ? `Trial · ${auth.billing.trialDaysRemaining ?? 0}d left`
      : auth.billing.plan === "pro"
        ? "Pro"
        : "Free";

  return (
    <Modal
      visible={visible}
      transparent
      animationType="slide"
      onRequestClose={onClose}
    >
      <Pressable style={styles.backdrop} onPress={onClose}>
        <Pressable style={styles.sheet} onPress={(e) => e.stopPropagation()}>
          <View style={styles.handle} />
          <View style={styles.row}>
            <Text style={styles.email} numberOfLines={1}>
              {auth.bypass ? "Not signed in" : (auth.session?.user.email ?? "")}
            </Text>
            <View style={styles.planChip}>
              <Text style={styles.planLabel}>{planLabel}</Text>
            </View>
          </View>
          <Text style={styles.syncNote}>
            Notes stay on this device for now. Cloud sync arrives with the
            native sync bridge.
          </Text>
          {!auth.bypass && (
            <Button
              label="Sign out"
              onPress={() => {
                onClose();
                void auth.signOut();
              }}
              size="small"
              style={styles.signOut}
              variant="ghost"
            />
          )}
        </Pressable>
      </Pressable>
    </Modal>
  );
}

const styles = StyleSheet.create({
  backdrop: {
    flex: 1,
    justifyContent: "flex-end",
    backgroundColor: Colors.scrim,
  },
  sheet: {
    backgroundColor: Colors.surface,
    borderTopLeftRadius: Radius.sheet,
    borderTopRightRadius: Radius.sheet,
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.xl,
    paddingTop: Spacing.sm,
  },
  handle: {
    alignSelf: "center",
    width: 40,
    height: 4,
    borderRadius: 2,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.border,
    marginBottom: Spacing.md,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: Spacing.sm,
  },
  email: {
    flex: 1,
    ...Typography.section,
    color: Colors.ink,
  },
  planChip: {
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accentSurface,
    paddingHorizontal: Spacing.sm,
    paddingVertical: Spacing.xs,
  },
  planLabel: {
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  syncNote: {
    marginTop: Spacing.md,
    ...Typography.body,
    color: Colors.muted,
  },
  signOut: {
    marginTop: Spacing.lg,
    alignSelf: "flex-start",
  },
});
