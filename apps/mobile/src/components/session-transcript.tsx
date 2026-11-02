import { useRef, type ReactNode } from "react";
import { FlatList, Text, View } from "react-native";

import { Spacing, Typography } from "@/constants/theme";
import {
  useSessionTranscripts,
  type TranscriptSegment,
} from "@/data/transcripts";
import { createStyleHook } from "@/settings/theme-provider";

export function SessionTranscript({
  sessionId,
  live,
  recordingDetails,
  focusSegmentId,
}: {
  sessionId: string;
  live?: {
    status: "connecting" | "live" | "fallback";
    text: string;
  };
  recordingDetails?: ReactNode;
  focusSegmentId?: string | null;
}) {
  const styles = useStyles();
  const { segments, isLoading, error } = useSessionTranscripts(sessionId, true);
  const listRef = useRef<FlatList<TranscriptSegment>>(null);
  const following = useRef(Boolean(live));
  const focusedSegment = useRef<string | null>(null);

  return (
    <FlatList
      ref={listRef}
      style={styles.list}
      contentContainerStyle={styles.content}
      data={segments}
      keyExtractor={(item) => item.id}
      onScroll={({
        nativeEvent: { contentOffset, contentSize, layoutMeasurement },
      }) => {
        following.current =
          contentSize.height - layoutMeasurement.height - contentOffset.y < 80;
      }}
      scrollEventThrottle={100}
      onContentSizeChange={() => {
        if (focusSegmentId && focusedSegment.current !== focusSegmentId) {
          const index = segments.findIndex(
            (segment) => segment.id === focusSegmentId,
          );
          if (index >= 0) {
            focusedSegment.current = focusSegmentId;
            listRef.current?.scrollToIndex({
              index,
              animated: true,
              viewPosition: 0.35,
            });
          }
        }
        if (live && following.current)
          listRef.current?.scrollToEnd({ animated: true });
      }}
      onScrollToIndexFailed={({ index }) => {
        setTimeout(
          () =>
            listRef.current?.scrollToIndex({
              index,
              animated: true,
              viewPosition: 0.35,
            }),
          100,
        );
      }}
      renderItem={({ item }) => (
        <View style={[styles.turn, item.id === focusSegmentId && styles.focus]}>
          <Text style={styles.speaker}>{item.speaker}</Text>
          <Text selectable style={styles.transcriptText}>
            {item.text}
          </Text>
        </View>
      )}
      ListEmptyComponent={
        !live?.text ? (
          <Text style={styles.hint}>
            {isLoading
              ? "Loading transcript…"
              : error
                ? "Couldn't load the transcript. Reopen the transcript to retry."
                : live
                  ? live.status === "fallback"
                    ? "Your recording will be transcribed after you stop listening."
                    : "Your transcript will appear here as you speak."
                  : "No transcript yet."}
          </Text>
        ) : null
      }
      ListFooterComponent={
        <View>
          {Boolean(live?.text) && (
            <View style={styles.turn}>
              <Text style={styles.speaker}>Speaking…</Text>
              <Text style={styles.hint}>{live?.text}</Text>
            </View>
          )}
          {recordingDetails}
        </View>
      }
    />
  );
}

const useStyles = createStyleHook((Colors) => ({
  list: { flex: 1 },
  content: {
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.lg,
  },
  turn: { paddingVertical: Spacing.sm, gap: Spacing.xs },
  speaker: { ...Typography.captionStrong, color: Colors.muted },
  transcriptText: { ...Typography.body, color: Colors.ink },
  hint: { ...Typography.caption, color: Colors.muted },
  focus: { backgroundColor: Colors.mutedSurface },
}));
