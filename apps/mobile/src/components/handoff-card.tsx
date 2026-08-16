import { Ionicons } from "@expo/vector-icons";
import { useState } from "react";
import { StyleSheet, Text, View } from "react-native";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Colors, Spacing, Typography } from "@/constants/theme";
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
    <Card style={styles.card} tone="muted">
      <View style={styles.statusRow}>
        <Ionicons
          name={
            status === "shared_unconfirmed" ? "send" : "phone-portrait-outline"
          }
          size={17}
          color={Colors.muted}
        />
        <Text style={styles.status}>{handoffStatusCopy(status)}</Text>
      </View>
      <Button
        label={status === "sharing" ? "Opening…" : "Send recording"}
        disabled={status === "sharing"}
        loading={status === "sharing"}
        onPress={() => void handleHandoff()}
        size="small"
        style={styles.button}
      />
    </Card>
  );
}

const styles = StyleSheet.create({
  card: {
    gap: Spacing.sm,
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
    padding: Spacing.md,
  },
  statusRow: {
    flexDirection: "row",
    alignItems: "flex-start",
    gap: Spacing.sm,
  },
  status: {
    flex: 1,
    ...Typography.caption,
    color: Colors.muted,
  },
  button: {
    alignSelf: "flex-start",
  },
});
