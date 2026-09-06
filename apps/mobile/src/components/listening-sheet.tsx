import { Ionicons } from "@expo/vector-icons";
import { useState } from "react";
import {
  ActivityIndicator,
  Pressable,
  StyleSheet,
  Text,
  View,
} from "react-native";
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from "react-native-reanimated";

import type {
  RecorderFailure,
  RecorderPhase,
} from "@/audio/use-session-recorder";
import { DancingSticks } from "@/components/dancing-sticks";
import {
  CornerCurve,
  LISTENING_CONTROL_HEIGHT,
  LISTENING_CONTROL_RADIUS,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";
import { createStyleHook, useColors } from "@/settings/theme-provider";

const DETAIL_HEIGHT = 124;

function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

function statusLabel(phase: RecorderPhase, durationMs: number): string {
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
      return "Recording needs to be saved";
    case "error":
      return "Recorder unavailable";
    case "saved":
      return "Recording saved";
    default:
      return "Getting ready…";
  }
}

function transcriptionLabel(status: "connecting" | "live" | "fallback") {
  switch (status) {
    case "live":
      return "Live transcription";
    case "fallback":
      return "Recording on this device";
    default:
      return "Connecting live transcript…";
  }
}

export function ListeningSheet({
  phase,
  failure,
  amplitude,
  durationMs,
  liveStatus,
  liveTranscript,
  onStop,
  onRetry,
  onOpenSettings,
}: {
  phase: RecorderPhase;
  failure: RecorderFailure | null;
  amplitude: number;
  durationMs: number;
  liveStatus: "connecting" | "live" | "fallback";
  liveTranscript: string;
  onStop: () => void;
  onRetry: () => void;
  onOpenSettings: () => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const [expanded, setExpanded] = useState(false);
  const detailHeight = useSharedValue(0);

  const toggle = () => {
    const next = !expanded;
    setExpanded(next);
    detailHeight.value = withTiming(next ? DETAIL_HEIGHT : 0, {
      duration: 240,
    });
  };

  const detailStyle = useAnimatedStyle(() => ({
    height: detailHeight.value,
  }));
  const permissionDenied =
    phase === "unavailable" && failure === "permission_denied";
  const recoverable = ["interrupted", "save_error", "error"].includes(phase);
  const handlePanelPress = permissionDenied
    ? onOpenSettings
    : recoverable
      ? onRetry
      : onStop;

  return (
    <View style={styles.sheet}>
      <Pressable hitSlop={12} onPress={toggle} style={styles.chevron}>
        <Ionicons
          name={expanded ? "chevron-down" : "chevron-up"}
          size={22}
          color={Colors.muted}
        />
      </Pressable>

      <Animated.View style={[styles.detail, detailStyle]}>
        <View style={styles.detailRow}>
          <View style={styles.recordingDot} />
          <Text style={styles.detailStatus}>
            {statusLabel(phase, durationMs)}
          </Text>
        </View>
        <Text style={styles.transcriptionStatus}>
          {transcriptionLabel(liveStatus)}
        </Text>
        <Text numberOfLines={2} style={styles.detailHint}>
          {liveTranscript ||
            "Leave your phone on the table and talk. Audio stays locally durable while Anarlog listens."}
        </Text>
      </Animated.View>

      <Pressable
        onPress={handlePanelPress}
        disabled={phase === "saving"}
        style={({ pressed }) => [styles.panel, pressed && styles.panelPressed]}
      >
        {phase === "saving" ? (
          <View style={styles.panelCenter}>
            <ActivityIndicator color={Colors.inkInverse} />
          </View>
        ) : permissionDenied ? (
          <View style={styles.panelCenter}>
            <Text style={styles.panelMessage}>
              Microphone access is off — open Settings
            </Text>
          </View>
        ) : phase === "interrupted" ? (
          <View style={styles.panelCenter}>
            <Text style={styles.panelMessage}>
              Interrupted — tap to save or retry
            </Text>
          </View>
        ) : phase === "save_error" ? (
          <View style={styles.panelCenter}>
            <Text style={styles.panelMessage}>
              Couldn't save — tap to retry
            </Text>
          </View>
        ) : phase === "error" ? (
          <View style={styles.panelCenter}>
            <Text style={styles.panelMessage}>Tap to recover recording</Text>
          </View>
        ) : (
          <DancingSticks
            amplitude={amplitude}
            color={Colors.inkInverse}
            height={36}
            width={80}
            stickWidth={3}
            gap={3}
          />
        )}
      </Pressable>
    </View>
  );
}

const useStyles = createStyleHook((Colors) => ({
  sheet: {
    borderWidth: StyleSheet.hairlineWidth,
    borderBottomWidth: 0,
    borderColor: Colors.border,
    borderTopLeftRadius: Radius.sheet,
    borderTopRightRadius: Radius.sheet,
    backgroundColor: Colors.background,
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.md,
    shadowColor: Colors.ink,
    shadowOffset: { width: 0, height: -4 },
    shadowOpacity: 0.06,
    shadowRadius: 12,
    elevation: 4,
  },
  chevron: {
    alignSelf: "center",
    paddingVertical: Spacing.sm,
  },
  detail: {
    overflow: "hidden",
  },
  detailRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
    paddingHorizontal: Spacing.sm,
  },
  recordingDot: {
    width: 10,
    height: 10,
    borderRadius: 5,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accent,
  },
  detailStatus: {
    ...Typography.bodyStrong,
    color: Colors.ink,
  },
  detailHint: {
    paddingHorizontal: Spacing.sm,
    paddingTop: Spacing.sm,
    ...Typography.caption,
    color: Colors.muted,
  },
  transcriptionStatus: {
    paddingHorizontal: Spacing.sm,
    paddingTop: Spacing.xs,
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  panel: {
    height: LISTENING_CONTROL_HEIGHT,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: LISTENING_CONTROL_RADIUS,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accent,
  },
  panelPressed: {
    opacity: 0.9,
  },
  panelCenter: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
  },
  panelMessage: {
    ...Typography.label,
    color: Colors.inkInverse,
  },
}));
