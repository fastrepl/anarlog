import {
  ActivityIndicator,
  Pressable,
  StyleSheet,
  Text,
  View,
} from "react-native";

import type {
  RecorderFailure,
  RecorderPhase,
} from "@/audio/use-session-recorder";
import { DancingSticks } from "@/components/dancing-sticks";
import { CornerCurve, Radius, Spacing, Typography } from "@/constants/theme";
import { createStyleHook, useColors } from "@/settings/theme-provider";

function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

function statusLabel(
  phase: RecorderPhase,
  failure: RecorderFailure | null,
  durationMs: number,
): string {
  switch (phase) {
    case "recording":
      return `Listening · ${formatDuration(durationMs)}`;
    case "saving":
      return "Saving recording…";
    case "unavailable":
      return "Microphone access needed";
    case "interrupted":
      return "Recording interrupted";
    case "save_error":
      return failure === "save_failed"
        ? "Free storage, then retry saving"
        : "Recording needs to be saved";
    case "error":
      return "Recorder unavailable";
    case "saved":
      return "Recording saved";
    default:
      return "Getting ready…";
  }
}

export function ListeningSheet({
  phase,
  failure,
  amplitude,
  durationMs,
  onStop,
  onRetry,
  onOpenSettings,
}: {
  phase: RecorderPhase;
  failure: RecorderFailure | null;
  amplitude: number;
  durationMs: number;
  onStop: () => void;
  onRetry: () => void;
  onOpenSettings: () => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const permissionDenied =
    phase === "unavailable" &&
    (failure === "permission_denied" ||
      failure === "notification_permission_denied");
  const recoverable = ["interrupted", "save_error", "error"].includes(phase);
  const handlePanelPress = permissionDenied
    ? onOpenSettings
    : recoverable
      ? onRetry
      : onStop;
  const label = statusLabel(phase, failure, durationMs);
  const actionLabel = permissionDenied
    ? "Settings"
    : recoverable
      ? "Retry"
      : "Stop";

  return (
    <View style={styles.dock}>
      <View style={styles.status}>
        {phase === "recording" ? (
          <DancingSticks
            amplitude={amplitude}
            color={Colors.accent}
            height={20}
            width={24}
            stickWidth={2}
            gap={2}
          />
        ) : phase === "saving" || phase === "starting" ? (
          <ActivityIndicator color={Colors.muted} size="small" />
        ) : null}
        <Text style={styles.statusText}>{label}</Text>
      </View>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={
          permissionDenied
            ? "Open recording settings"
            : recoverable
              ? "Recover recording"
              : "Stop listening"
        }
        accessibilityState={{ disabled: phase === "saving" }}
        onPress={handlePanelPress}
        disabled={phase === "saving"}
        style={({ pressed }) => [styles.control, pressed && styles.pressed]}
      >
        {!permissionDenied && !recoverable && <View style={styles.stopIcon} />}
        <Text style={styles.controlText}>{actionLabel}</Text>
      </Pressable>
    </View>
  );
}

const useStyles = createStyleHook((Colors) => ({
  dock: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.md,
    paddingHorizontal: Spacing.md,
    paddingVertical: Spacing.sm,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
  },
  status: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
  },
  statusText: {
    flexShrink: 1,
    ...Typography.caption,
    color: Colors.muted,
    fontVariant: ["tabular-nums"],
  },
  control: {
    minHeight: 44,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.sm,
    paddingHorizontal: Spacing.md,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.surface,
  },
  pressed: { backgroundColor: Colors.accentSurface },
  controlText: { ...Typography.bodyStrong, color: Colors.ink },
  stopIcon: {
    width: 12,
    height: 12,
    borderRadius: 2,
    backgroundColor: Colors.accent,
  },
}));
