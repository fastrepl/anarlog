import { Ionicons } from "@expo/vector-icons";
import { Text, View } from "react-native";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Spacing, Typography } from "@/constants/theme";
import { createStyleHook, useColors } from "@/settings/theme-provider";

export function RemoteAudioCard({
  errorMessage,
  loading,
  cloudAvailable,
  onDownloadRecording,
  onChooseRecording,
}: {
  errorMessage: string | null;
  loading: boolean;
  cloudAvailable: boolean;
  onDownloadRecording: () => void;
  onChooseRecording: () => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  return (
    <Card style={styles.card} tone="muted">
      <Ionicons
        name={
          cloudAvailable ? "cloud-download-outline" : "cloud-offline-outline"
        }
        size={17}
        color={Colors.muted}
      />
      <View style={styles.copy}>
        <Text style={styles.title}>Recording not on this phone</Text>
        <Text style={styles.description}>
          {cloudAvailable
            ? "Download the encrypted recording from your Anarlog sync."
            : "Anarlog has the meeting details, but the recording has not synced yet."}
        </Text>
        {errorMessage && <Text style={styles.error}>{errorMessage}</Text>}
        <Button
          label={cloudAvailable ? "Download recording" : "Choose recording"}
          loading={loading}
          onPress={cloudAvailable ? onDownloadRecording : onChooseRecording}
          size="small"
          style={styles.action}
          variant={cloudAvailable ? "primary" : "outline"}
        />
        {cloudAvailable && (
          <Button
            label="Choose file instead"
            disabled={loading}
            onPress={onChooseRecording}
            size="small"
            style={styles.fallbackAction}
            variant="ghost"
          />
        )}
      </View>
    </Card>
  );
}

const useStyles = createStyleHook((Colors) => ({
  card: {
    flexDirection: "row",
    alignItems: "flex-start",
    gap: Spacing.sm,
    marginHorizontal: Spacing.md,
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
  fallbackAction: {
    alignSelf: "flex-start",
    marginTop: Spacing.xs,
  },
}));
