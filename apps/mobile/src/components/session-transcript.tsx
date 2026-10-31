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
}: {
  sessionId: string;
  live?: {
    status: "connecting" | "live" | "fallback";
    text: string;
  };
  recordingDetails?: ReactNode;
}) {
  const styles = useStyles();
  const { segments, isLoading, error } = useSessionTranscripts(sessionId, true);
  const listRef = useRef<FlatList<TranscriptSegment>>(null);
  const following = useRef(Boolean(live));

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
        if (live && following.current)
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
        !live?.text && live?.status !== "fallback" ? (
          <Text style={styles.hint}>
            {isLoading
              ? "Loading transcript…"
              : error
                ? "Couldn't load the transcript. Reopen the transcript to retry."
                : live
                  ? live.status === "connecting"
                    ? "Preparing live transcription…"
                    : "Your transcript will appear here as you speak."
                  : "No transcript yet."}
          </Text>
        ) : null
      }
      ListFooterComponent={
        <View>
          {live?.status === "fallback" && (
            <View style={styles.turn}>
              <Text style={styles.speaker}>Live transcription interrupted</Text>
              <Text style={styles.hint}>
                {live.text ||
                  "Your recording will be transcribed after you stop listening."}
              </Text>
            </View>
          )}
          {live?.status !== "fallback" && Boolean(live?.text) && (
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
}));
