import { Ionicons } from "@expo/vector-icons";
import { StyleSheet, Text, View } from "react-native";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Colors, Spacing, Typography } from "@/constants/theme";

export function RemoteAudioCard({
  errorMessage,
  loading,
  onChooseRecording,
}: {
  errorMessage: string | null;
  loading: boolean;
  onChooseRecording: () => void;
}) {
  return (
    <Card style={styles.card} tone="muted">
      <Ionicons name="cloud-offline-outline" size={17} color={Colors.muted} />
      <View style={styles.copy}>
        <Text style={styles.title}>Recording not on this phone</Text>
        <Text style={styles.description}>
          Anarlog has the meeting details, but this phone does not have the
          audio file.
        </Text>
        {errorMessage && <Text style={styles.error}>{errorMessage}</Text>}
        <Button
          label="Choose recording"
          loading={loading}
          onPress={onChooseRecording}
          size="small"
          style={styles.action}
          variant="outline"
        />
      </View>
    </Card>
  );
}

const styles = StyleSheet.create({
  card: {
    flexDirection: "row",
    alignItems: "flex-start",
    gap: Spacing.sm,
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
    padding: Spacing.md,
  },
  copy: {
    flex: 1,
  },
  title: {
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  description: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
  error: {
    marginTop: Spacing.sm,
    ...Typography.caption,
    color: Colors.destructive,
  },
  action: {
    alignSelf: "flex-start",
    marginTop: Spacing.md,
  },
});
