import { Ionicons } from "@expo/vector-icons";
import { useState } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { Colors, CornerCurve, Radius, Spacing } from "@/constants/theme";
import { handoffRecording } from "@/data/handoff";
import { handoffStatusCopy, type HandoffStatus } from "@/data/handoff-status";

export function HandoffCard({
  uri,
  filename,
}: {
  uri: string;
  filename: string;
}) {
  const [status, setStatus] = useState<HandoffStatus>("local");

  const handleHandoff = async () => {
    if (status === "sharing") return;
    setStatus("sharing");
    setStatus(await handoffRecording(uri, filename));
  };

  return (
    <View style={styles.card}>
      <View style={styles.statusRow}>
        <Ionicons
          name={
            status === "shared_unconfirmed" ? "send" : "phone-portrait-outline"
          }
          size={17}
          color={Colors.ink}
        />
        <Text style={styles.status}>{handoffStatusCopy(status)}</Text>
      </View>
      <Pressable
        disabled={status === "sharing"}
        onPress={() => void handleHandoff()}
        style={({ pressed }) => [
          styles.button,
          (pressed || status === "sharing") && styles.pressed,
        ]}
      >
        <Text style={styles.buttonLabel}>
          {status === "sharing" ? "Opening…" : "Send recording"}
        </Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    gap: Spacing.sm,
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
    padding: Spacing.md,
    borderWidth: 1,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
  },
  statusRow: {
    flexDirection: "row",
    alignItems: "flex-start",
    gap: Spacing.sm,
  },
  status: {
    flex: 1,
    fontSize: 13,
    lineHeight: 18,
    color: Colors.muted,
  },
  button: {
    alignSelf: "flex-start",
    paddingHorizontal: Spacing.md,
    paddingVertical: Spacing.sm,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.ink,
  },
  pressed: {
    opacity: 0.65,
  },
  buttonLabel: {
    fontSize: 13,
    fontWeight: "600",
    color: Colors.inkInverse,
  },
});
