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

import type { RecorderPhase } from "@/audio/use-session-recorder";
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
      return "Microphone unavailable";
    case "error":
      return "Couldn't save the recording";
    default:
      return "Getting ready…";
  }
}

export function ListeningSheet({
  phase,
  levels,
  durationMs,
  onStop,
}: {
  phase: RecorderPhase;
  levels: number[] | null;
  durationMs: number;
  onStop: () => void;
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
          Leave your phone on the table and talk. The transcript is generated
          once the recording syncs.
        </Text>
      </Animated.View>

      <Pressable
        onPress={onStop}
        disabled={phase === "saving"}
        style={({ pressed }) => [styles.panel, pressed && styles.panelPressed]}
      >
        {phase === "saving" ? (
          <View style={styles.panelCenter}>
            <ActivityIndicator color={Colors.inkInverse} />
          </View>
        ) : phase === "error" ? (
          <View style={styles.panelCenter}>
            <Text style={styles.panelMessage}>
              Couldn't save — tap to dismiss
            </Text>
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
