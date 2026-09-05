import { Ionicons } from "@expo/vector-icons";
import { useAudioPlayer, useAudioPlayerStatus } from "expo-audio";
import { Pressable, StyleSheet, Text } from "react-native";

import { CornerCurve, Radius, Spacing, Typography } from "@/constants/theme";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";
import { createStyleHook, useColors } from "@/settings/theme-provider";

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

function formatSeconds(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

export function AudioChip({
  uri,
  filename,
  sizeBytes,
}: {
  uri: string;
  filename: string;
  sizeBytes: number;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const player = useAudioPlayer(uri);
  const status = useAudioPlayerStatus(player);

  useMountEffect(() => {
    let lastError: string | null = null;
    const subscription = player.addListener(
      "playbackStatusUpdate",
      (nextStatus) => {
        const errorKey = nextStatus.error
          ? nextStatus.error
          : nextStatus.mediaServicesDidReset
            ? "media_services_reset"
            : null;
        if (!errorKey || errorKey === lastError) return;

        lastError = errorKey;
        captureOperationalError(
          new Error(nextStatus.error ?? "Audio media services reset"),
          {
            operation: nextStatus.mediaServicesDidReset
              ? "audio_playback_media_services_reset"
              : "audio_playback",
            level: nextStatus.mediaServicesDidReset ? "warning" : "error",
          },
        );
      },
    );
    return () => subscription.remove();
  });

  const toggle = async () => {
    try {
      if (status.playing) {
        player.pause();
        return;
      }
      if (status.duration > 0 && status.currentTime >= status.duration - 0.05) {
        await player.seekTo(0);
      }
      player.play();
    } catch (error) {
      captureOperationalError(error, {
        operation: "audio_playback_control",
      });
    }
  };

  const detail =
    status.playing || status.currentTime > 0
      ? `${formatSeconds(status.currentTime)} / ${formatSeconds(status.duration)}`
      : formatBytes(sizeBytes);

  return (
    <Pressable
      onPress={() => void toggle()}
      style={({ pressed }) => [styles.chip, pressed && styles.pressed]}
    >
      <Ionicons
        name={status.playing ? "pause" : "play"}
        size={14}
        color={Colors.accent}
      />
      <Text style={styles.label} numberOfLines={1}>
        {filename} · {detail}
      </Text>
    </Pressable>
  );
}

const useStyles = createStyleHook((Colors) => ({
  chip: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.xs,
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
    paddingHorizontal: Spacing.sm,
    paddingVertical: Spacing.xs,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.mutedSurface,
    alignSelf: "flex-start",
  },
  pressed: {
    opacity: 0.6,
  },
  label: {
    ...Typography.captionStrong,
    color: Colors.ink,
    maxWidth: 180,
  },
}));
