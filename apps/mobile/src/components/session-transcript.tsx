import { useMutation } from "@tanstack/react-query";
import { useRef, useState, type ReactNode } from "react";
import {
  ActivityIndicator,
  FlatList,
  KeyboardAvoidingView,
  Modal,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";

import { Spacing, Typography } from "@/constants/theme";
import {
  saveTranscriptSegmentEdit,
  useSessionTranscripts,
  type TranscriptSegment,
} from "@/data/transcripts";
import { createStyleHook, useColors } from "@/settings/theme-provider";

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
  const [editing, setEditing] = useState<TranscriptSegment | null>(null);

  return (
    <>
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
            contentSize.height - layoutMeasurement.height - contentOffset.y <
            80;
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
          <View
            style={[styles.turn, item.id === focusSegmentId && styles.focus]}
          >
            <Text style={styles.speaker}>{item.speaker}</Text>
            <Text selectable style={styles.transcriptText}>
              {item.text}
            </Text>
            {!live && (
              <Pressable
                accessibilityLabel={`Edit transcript segment, ${item.speaker}`}
                accessibilityRole="button"
                onPress={() => setEditing(item)}
                style={({ pressed }) => [
                  styles.editButton,
                  pressed && styles.pressed,
                ]}
              >
                <Text style={styles.editLabel}>
                  {item.originalText === undefined ? "Edit" : "Edited · Edit"}
                </Text>
              </Pressable>
            )}
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
      {editing && (
        <TranscriptSegmentEditor
          key={editing.id}
          sessionId={sessionId}
          segment={editing}
          onClose={() => setEditing(null)}
        />
      )}
    </>
  );
}

function TranscriptSegmentEditor({
  sessionId,
  segment,
  onClose,
}: {
  sessionId: string;
  segment: TranscriptSegment;
  onClose: () => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const [draft, setDraft] = useState(segment.text);
  const save = useMutation({
    mutationFn: () => saveTranscriptSegmentEdit(sessionId, segment, draft),
    onSuccess: onClose,
  });
  return (
    <Modal
      animationType="slide"
      onRequestClose={onClose}
      presentationStyle="pageSheet"
      visible
    >
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : undefined}
        style={styles.editor}
      >
        <View style={styles.editorHeader}>
          <Text accessibilityRole="header" style={styles.editorTitle}>
            Edit transcript
          </Text>
          <Pressable accessibilityRole="button" onPress={onClose}>
            <Text style={styles.editLabel}>Cancel</Text>
          </Pressable>
        </View>
        <Text style={styles.hint}>
          The original recognition and each edit are retained in this meeting’s
          transcript history.
        </Text>
        <TextInput
          accessibilityLabel="Transcript segment text"
          autoCapitalize="sentences"
          multiline
          onChangeText={setDraft}
          textAlignVertical="top"
          value={draft}
          style={styles.editorInput}
        />
        {save.error && (
          <Text accessibilityRole="alert" style={styles.editorError}>
            {save.error instanceof Error
              ? save.error.message
              : "Couldn’t save this transcript edit."}
          </Text>
        )}
        <Pressable
          accessibilityRole="button"
          accessibilityState={{ disabled: save.isPending || !draft.trim() }}
          disabled={save.isPending || !draft.trim()}
          onPress={() => save.mutate()}
          style={({ pressed }) => [
            styles.saveButton,
            { backgroundColor: Colors.accent },
            pressed && styles.pressed,
          ]}
        >
          {save.isPending ? (
            <ActivityIndicator color={Colors.background} />
          ) : (
            <Text style={styles.saveLabel}>Save transcript edit</Text>
          )}
        </Pressable>
      </KeyboardAvoidingView>
    </Modal>
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
  editButton: { alignSelf: "flex-start", paddingVertical: Spacing.xs },
  editLabel: { ...Typography.captionStrong, color: Colors.accent },
  saveLabel: { ...Typography.bodyStrong, color: Colors.background },
  editor: {
    flex: 1,
    padding: Spacing.lg,
    paddingTop: Spacing.xl,
    gap: Spacing.md,
    backgroundColor: Colors.background,
  },
  editorHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  editorTitle: { ...Typography.title, color: Colors.ink },
  editorInput: {
    minHeight: 150,
    padding: Spacing.md,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: 12,
    color: Colors.ink,
    ...Typography.body,
  },
  editorError: { ...Typography.caption, color: Colors.destructive },
  saveButton: {
    minHeight: 48,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 12,
    paddingHorizontal: Spacing.md,
  },
  pressed: { opacity: 0.72 },
  focus: { backgroundColor: Colors.mutedSurface },
}));
