import { Modal, Pressable, StyleSheet, Text, View } from "react-native";

import { useAuth } from "@/auth/context";
import { Colors, CornerCurve, Radius, Spacing } from "@/constants/theme";

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
            <Pressable
              onPress={() => {
                onClose();
                void auth.signOut();
              }}
              style={({ pressed }) => [
                styles.signOut,
                pressed && styles.signOutPressed,
              ]}
            >
              <Text style={styles.signOutLabel}>Sign out</Text>
            </Pressable>
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
    backgroundColor: "rgba(17, 17, 17, 0.35)",
  },
  sheet: {
    backgroundColor: Colors.paper,
    borderTopLeftRadius: Radius.pill,
    borderTopRightRadius: Radius.pill,
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
    fontSize: 16,
    fontWeight: "600",
    color: Colors.ink,
  },
  planChip: {
    borderWidth: 1.5,
    borderColor: Colors.ink,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    paddingHorizontal: Spacing.sm,
    paddingVertical: 2,
  },
  planLabel: {
    fontSize: 12,
    fontWeight: "600",
    color: Colors.ink,
  },
  syncNote: {
    marginTop: Spacing.md,
    fontSize: 14,
    lineHeight: 20,
    color: Colors.muted,
  },
  signOut: {
    marginTop: Spacing.lg,
    alignSelf: "flex-start",
  },
  signOutPressed: {
    opacity: 0.6,
  },
  signOutLabel: {
    fontSize: 15,
    fontWeight: "600",
    color: Colors.accent,
  },
});
