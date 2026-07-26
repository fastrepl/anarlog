import { Ionicons } from "@expo/vector-icons";
import { useAudioPlayer, useAudioPlayerStatus } from "expo-audio";
import { Pressable, StyleSheet, Text } from "react-native";

import { Colors, Radius, Spacing } from "@/constants/theme";

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
  pending,
}: {
  uri: string;
  filename: string;
  sizeBytes: number;
  pending: boolean;
}) {
  const player = useAudioPlayer(uri);
  const status = useAudioPlayerStatus(player);

  const toggle = () => {
    if (status.playing) {
      player.pause();
      return;
    }
    if (status.duration > 0 && status.currentTime >= status.duration - 0.05) {
      void player.seekTo(0);
    }
    player.play();
  };

  const detail =
    status.playing || status.currentTime > 0
      ? `${formatSeconds(status.currentTime)} / ${formatSeconds(status.duration)}`
      : formatBytes(sizeBytes);

  return (
    <Pressable
      onPress={toggle}
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
      {pending && <Text style={styles.status}>Transcribes after sync</Text>}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  chip: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.xs,
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
    paddingHorizontal: Spacing.sm,
    paddingVertical: Spacing.xs,
    borderWidth: 1.5,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    alignSelf: "flex-start",
  },
  pressed: {
    opacity: 0.6,
  },
  label: {
    fontSize: 12,
    fontWeight: "600",
    color: Colors.ink,
    maxWidth: 180,
  },
  status: {
    fontSize: 12,
    color: Colors.muted,
  },
});
