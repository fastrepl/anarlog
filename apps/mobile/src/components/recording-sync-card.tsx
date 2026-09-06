import { Ionicons } from "@expo/vector-icons";
import { useState } from "react";
import { Text, View } from "react-native";

import { useAuth } from "@/auth/context";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Spacing, Typography } from "@/constants/theme";
import {
  retrySessionAudioUpload,
  type SessionAudio,
} from "@/data/audio-catalog";
import { captureOperationalError } from "@/lib/error-reporting";
import { createStyleHook, useColors } from "@/settings/theme-provider";

const presentation = {
  queued: {
    icon: "time-outline" as const,
    text: "Saved locally · waiting to back up",
  },
  uploading: {
    icon: "cloud-upload-outline" as const,
    text: "Backing up encrypted recording…",
  },
  synced: {
    icon: "cloud-done-outline" as const,
    text: "Encrypted recording backed up",
  },
  failed: {
    icon: "alert-circle-outline" as const,
    text: "Saved locally · backup needs attention",
  },
};

export function RecordingSyncCard({ audio }: { audio: SessionAudio }) {
  const styles = useStyles();
  const { billing } = useAuth();
  const Colors = useColors();
  const [retrying, setRetrying] = useState(false);
  const [retryError, setRetryError] = useState(false);
  const status =
    !billing.isPro && audio.deliveryState !== "synced"
      ? {
          icon: "phone-portrait-outline" as const,
          text: "Saved on this device",
        }
      : presentation[audio.deliveryState];
  const waitingToRetry = billing.isPro && audio.uploadPhase === "retry_wait";
  const canRetry =
    billing.isPro && (audio.deliveryState === "failed" || waitingToRetry);

  const handleRetry = async () => {
    if (retrying) return;
    setRetrying(true);
    setRetryError(false);
    try {
      await retrySessionAudioUpload(audio.attachmentId);
    } catch (error) {
      setRetryError(true);
      captureOperationalError(error, {
        operation: "session_audio_upload_retry",
      });
    } finally {
      setRetrying(false);
    }
  };

  return (
    <Card
      style={styles.card}
      tone={
        billing.isPro && audio.deliveryState === "failed" ? "alert" : "muted"
      }
    >
      <View style={styles.statusRow}>
        <Ionicons name={status.icon} size={17} color={Colors.muted} />
        <Text accessibilityLiveRegion="polite" style={styles.status}>
          {retryError
            ? "Could not restart the backup. Try again."
            : waitingToRetry
              ? "Saved locally · retrying when online"
              : status.text}
        </Text>
      </View>
      {canRetry && (
        <Button
          label="Retry now"
          loading={retrying}
          onPress={() => void handleRetry()}
          size="small"
          style={styles.button}
          variant="outline"
        />
      )}
    </Card>
  );
}

const useStyles = createStyleHook((Colors) => ({
  card: {
    gap: Spacing.sm,
    marginHorizontal: Spacing.md,
    marginTop: Spacing.sm,
    padding: Spacing.md,
  },
  statusRow: {
    flexDirection: "row",
    alignItems: "center",
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
}));
