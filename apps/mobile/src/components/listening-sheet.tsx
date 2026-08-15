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
import { Waveform } from "@/components/waveform";
import { Colors, CornerCurve, Radius, Spacing } from "@/constants/theme";

const DETAIL_HEIGHT = 96;

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

export function ListeningSheet({
  phase,
  failure,
  levels,
  durationMs,
  onStop,
  onRetry,
  onOpenSettings,
}: {
  phase: RecorderPhase;
  failure: RecorderFailure | null;
  levels: number[] | null;
  durationMs: number;
  onStop: () => void;
  onRetry: () => void;
  onOpenSettings: () => void;
}) {
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
          color={Colors.ink}
        />
      </Pressable>

      <Animated.View style={[styles.detail, detailStyle]}>
        <View style={styles.detailRow}>
          <View style={styles.recordingDot} />
          <Text style={styles.detailStatus}>
            {statusLabel(phase, durationMs)}
          </Text>
        </View>
        <Text style={styles.detailHint}>
          Leave your phone on the table and talk. Audio saves locally first;
          transcription starts after you stop.
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
          <Waveform levels={levels} />
        )}
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  sheet: {
    borderWidth: 1.5,
    borderBottomWidth: 0,
    borderColor: Colors.ink,
    borderTopLeftRadius: Radius.pill,
    borderTopRightRadius: Radius.pill,
    backgroundColor: Colors.paper,
    paddingHorizontal: Spacing.md,
    paddingBottom: Spacing.md,
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
    fontSize: 15,
    fontWeight: "600",
    color: Colors.ink,
  },
  detailHint: {
    paddingHorizontal: Spacing.sm,
    paddingTop: Spacing.sm,
    fontSize: 14,
    lineHeight: 20,
    color: Colors.muted,
  },
  panel: {
    borderRadius: Radius.card + 4,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accent,
    paddingVertical: Spacing.md,
  },
  panelPressed: {
    opacity: 0.9,
  },
  panelCenter: {
    height: 72,
    alignItems: "center",
    justifyContent: "center",
  },
  panelMessage: {
    fontSize: 15,
    fontWeight: "600",
    color: Colors.inkInverse,
  },
});
