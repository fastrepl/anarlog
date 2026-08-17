import { Ionicons } from "@expo/vector-icons";
import { StyleSheet, Text, View } from "react-native";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Colors, Spacing, Typography } from "@/constants/theme";

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

export function NoteAttachmentCard({
  availableLocally,
  cloudAvailable,
  errorMessage,
  filename,
  loading,
  onDownload,
  onShare,
  sizeBytes,
}: {
  availableLocally: boolean;
  cloudAvailable: boolean;
  errorMessage: string | null;
  filename: string;
  loading: boolean;
  onDownload: () => void;
  onShare: () => void;
  sizeBytes: number;
}) {
  return (
    <Card style={styles.card} tone="muted">
      <View style={styles.icon}>
        <Ionicons
          name={availableLocally ? "document-attach-outline" : "cloud-outline"}
          size={18}
          color={Colors.muted}
        />
      </View>
      <View style={styles.copy}>
        <Text numberOfLines={1} style={styles.title}>
          {filename}
        </Text>
        <Text style={styles.description}>
          {availableLocally
            ? formatBytes(sizeBytes)
            : cloudAvailable
              ? `${formatBytes(sizeBytes)} · Available from sync`
              : `${formatBytes(sizeBytes)} · Waiting for sync`}
        </Text>
        {errorMessage && <Text style={styles.error}>{errorMessage}</Text>}
      </View>
      <Button
        disabled={!availableLocally && !cloudAvailable}
        label={
          availableLocally ? "Share" : cloudAvailable ? "Download" : "Pending"
        }
        loading={loading}
        onPress={availableLocally ? onShare : onDownload}
        size="small"
        variant={availableLocally ? "outline" : "primary"}
      />
    </Card>
  );
}

const styles = StyleSheet.create({
  card: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
    padding: Spacing.sm,
  },
  icon: {
    width: 32,
    height: 32,
    alignItems: "center",
    justifyContent: "center",
  },
  copy: {
    flex: 1,
    minWidth: 0,
  },
  title: {
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  description: {
    marginTop: 2,
    ...Typography.caption,
    color: Colors.muted,
  },
  error: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.destructive,
  },
});
