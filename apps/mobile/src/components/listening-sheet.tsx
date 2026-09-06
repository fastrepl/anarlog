import { BottomSheet, RNHostView } from "@expo/ui";
import { Ionicons } from "@expo/vector-icons";
import { useRef, useState, type ReactNode } from "react";
import {
  ActivityIndicator,
  FlatList,
  Keyboard,
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
import {
  CornerCurve,
  LISTENING_CONTROL_HEIGHT,
  LISTENING_CONTROL_RADIUS,
  Spacing,
  Typography,
} from "@/constants/theme";
import type { TranscriptSegment } from "@/data/transcripts";
import { createStyleHook, useColors } from "@/settings/theme-provider";

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
  active,
  phase,
  failure,
  amplitude,
  durationMs,
  liveStatus,
  liveTranscript,
  transcripts,
  recordingDetails,
  onStop,
  onRetry,
  onOpenSettings,
}: {
  active: boolean;
  phase: RecorderPhase;
  failure: RecorderFailure | null;
  amplitude: number;
  durationMs: number;
  liveStatus: "connecting" | "live" | "fallback";
  liveTranscript: string;
  transcripts: TranscriptSegment[];
  recordingDetails: ReactNode;
  onStop: () => void;
  onRetry: () => void;
  onOpenSettings: () => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const [expanded, setExpanded] = useState(false);
  const listRef = useRef<FlatList<TranscriptSegment>>(null);
  const following = useRef(active);
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
  const control = (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={
        permissionDenied
          ? "Open recording settings"
          : recoverable
            ? "Recover recording"
            : "Stop listening"
      }
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
            {failure === "notification_permission_denied"
              ? "Allow recording notifications in Settings"
              : "Microphone access is off — open Settings"}
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
          <Text style={styles.panelMessage}>Couldn't save — tap to retry</Text>
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
  );
  const label = active ? statusLabel(phase, durationMs) : "Transcript";

  return (
    <>
      <View style={styles.dock}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Open full transcript"
          onPress={() => {
            Keyboard.dismiss();
            following.current = active;
            setExpanded(true);
          }}
          style={styles.heading}
        >
          {active && <View style={styles.recordingDot} />}
          <Text style={styles.headingText}>{label}</Text>
          <Ionicons name="chevron-up" size={20} color={Colors.muted} />
        </Pressable>
        {active && control}
      </View>
      <BottomSheet
        isPresented={expanded}
        onDismiss={() => setExpanded(false)}
        snapPoints={["half", "full"]}
        contentPadding={0}
      >
        <RNHostView>
          <View style={styles.content}>
            <View style={styles.heading}>
              {active && <View style={styles.recordingDot} />}
              <Text style={styles.headingText}>{label}</Text>
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Close transcript"
                hitSlop={12}
                onPress={() => setExpanded(false)}
              >
                <Ionicons name="chevron-down" size={22} color={Colors.muted} />
              </Pressable>
            </View>
            <FlatList
              ref={listRef}
              style={styles.list}
              contentContainerStyle={styles.transcriptContent}
              data={transcripts}
              keyExtractor={(item) => item.id}
              onScroll={({
                nativeEvent: { contentOffset, contentSize, layoutMeasurement },
              }) => {
                following.current =
                  contentSize.height -
                    layoutMeasurement.height -
                    contentOffset.y <
                  80;
              }}
              scrollEventThrottle={100}
              onContentSizeChange={() => {
                if (active && following.current)
                  listRef.current?.scrollToEnd({ animated: true });
              }}
              renderItem={({ item }) => (
                <View style={styles.turn}>
                  <Text style={styles.speaker}>{item.speaker}</Text>
                  <Text selectable style={styles.transcriptText}>
                    {item.text}
                  </Text>
                </View>
              )}
              ListEmptyComponent={
                !liveTranscript ? (
                  <Text style={styles.hint}>
                    {active
                      ? liveStatus === "fallback"
                        ? "Your recording will be transcribed after you stop listening."
                        : "Your transcript will appear here as you speak."
                      : "No transcript yet."}
                  </Text>
                ) : null
              }
              ListFooterComponent={
                <View>
                  {active && liveTranscript !== "" && (
                    <View style={styles.turn}>
                      <Text style={styles.speaker}>Speaking…</Text>
                      <Text style={styles.hint}>{liveTranscript}</Text>
                    </View>
                  )}
                  {!active && recordingDetails}
                </View>
              }
            />
            {active && (
              <View style={styles.controls}>
                <Text style={styles.hint}>
                  {transcriptionLabel(liveStatus)}
                </Text>
                {control}
              </View>
            )}
          </View>
        </RNHostView>
      </BottomSheet>
    </>
  );
}

const useStyles = createStyleHook((Colors) => ({
  dock: {
    paddingHorizontal: Spacing.md,
    paddingBottom: Spacing.sm,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
  },
  content: { flex: 1 },
  heading: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
    padding: Spacing.md,
  },
  headingText: { flex: 1, ...Typography.bodyStrong, color: Colors.ink },
  list: { flex: 1 },
  transcriptContent: {
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.lg,
  },
  turn: { paddingVertical: Spacing.sm, gap: Spacing.xs },
  speaker: { ...Typography.captionStrong, color: Colors.muted },
  transcriptText: { ...Typography.body, color: Colors.ink },
  hint: { ...Typography.caption, color: Colors.muted },
  controls: { padding: Spacing.md, gap: Spacing.sm },
  recordingDot: {
    width: 10,
    height: 10,
    borderRadius: 5,
    backgroundColor: Colors.accent,
  },
  panel: {
    height: LISTENING_CONTROL_HEIGHT,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: LISTENING_CONTROL_RADIUS,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.accent,
  },
  panelPressed: { opacity: 0.9 },
  panelCenter: { flex: 1, alignItems: "center", justifyContent: "center" },
  panelMessage: { ...Typography.label, color: Colors.inkInverse },
}));
