import { Ionicons } from "@expo/vector-icons";
import { File, Paths } from "expo-file-system";
import { useLocalSearchParams, useRouter } from "expo-router";
import { useEffect, useRef, useState } from "react";
import {
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { useSessionRecorder } from "@/audio/use-session-recorder";
import { AudioChip } from "@/components/audio-chip";
import { ListeningSheet } from "@/components/listening-sheet";
import { Colors, Spacing } from "@/constants/theme";
import { useSessionAudio } from "@/data/audio-catalog";
import {
  deleteSession,
  saveSessionNote,
  saveSessionTitle,
  useSessionDetail,
} from "@/data/session";
import { transcribeSession, useTranscriptionState } from "@/data/transcribe";
import { useSessionTranscripts } from "@/data/transcripts";
import { confirmDestructive } from "@/lib/confirm";
import { captureOperationalError } from "@/lib/error-reporting";

export default function NoteScreen() {
  const router = useRouter();
  const { id, listen } = useLocalSearchParams<{
    id: string;
    listen?: string;
  }>();
  const { data, isLoading } = useSessionDetail(id);
  const audio = useSessionAudio(id);
  const transcripts = useSessionTranscripts(id);
  const transcription = useTranscriptionState(id);
  const [listening, setListening] = useState(listen === "1");
  const recorder = useSessionRecorder(id, listening);

  const dataRef = useRef(data);
  dataRef.current = data;
  const draftRef = useRef<{ title?: string; body?: string }>({});
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // The live query lags our own writes, so a body-only flush would otherwise
  // resend the pre-edit title and undo the title we just persisted.
  const savedTitleRef = useRef<string | null>(null);
  if (savedTitleRef.current !== null && data?.title === savedTitleRef.current) {
    savedTitleRef.current = null;
  }

  const flush = () => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const draft = draftRef.current;
    draftRef.current = {};
    const current = dataRef.current;
    if (!current) return;
    if (draft.body !== undefined) {
      const title = draft.title ?? savedTitleRef.current ?? current.title;
      savedTitleRef.current = title;
      void saveSessionNote(id, {
        title,
        bodyText: draft.body,
        bodyFormat: current.bodyFormat,
      }).catch((error) => {
        captureOperationalError(error, {
          operation: "session_note_save",
          tags: {
            body_format: current.bodyFormat,
            edit_type: "body",
          },
        });
      });
    } else if (draft.title !== undefined) {
      savedTitleRef.current = draft.title;
      void saveSessionTitle(id, draft.title).catch((error) => {
        captureOperationalError(error, {
          operation: "session_note_save",
          tags: { edit_type: "title" },
        });
      });
    }
  };

  useEffect(() => flush, [id]);

  const onEdit = (patch: Partial<{ title: string; body: string }>) => {
    draftRef.current = { ...draftRef.current, ...patch };
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(flush, 500);
  };

  const handleBack = async () => {
    // Never lose a live recording to navigation — stop and save first.
    if (recorder.phase === "recording" || recorder.phase === "starting") {
      await recorder.stop();
    }
    flush();
    if (router.canGoBack()) router.back();
    else router.replace("/");
  };

  const handleStop = async () => {
    if (recorder.phase === "error" || recorder.phase === "unavailable") {
      setListening(false);
      return;
    }
    const result = await recorder.stop();
    if (result !== "failed") setListening(false);
  };

  const handleDelete = async () => {
    const confirmed = await confirmDestructive(
      `Delete "${data?.title || "Untitled"}"?`,
      "Delete",
    );
    if (!confirmed) return;
    // Same window as handleBack: unmounting during startup would let the hook's
    // own teardown save audio against an already-tombstoned session.
    if (recorder.phase === "recording" || recorder.phase === "starting") {
      await recorder.stop();
    }
    draftRef.current = {};
    try {
      await deleteSession(id);
      if (router.canGoBack()) router.back();
      else router.replace("/");
    } catch (error) {
      captureOperationalError(error, {
        operation: "session_delete",
        tags: { entry_point: "mobile_note" },
      });
    }
  };

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        <Pressable hitSlop={8} onPress={() => void handleBack()}>
          <Ionicons name="arrow-back" size={24} color={Colors.ink} />
        </Pressable>
        <Pressable hitSlop={8} onPress={() => void handleDelete()}>
          <Ionicons name="trash-outline" size={20} color={Colors.ink} />
        </Pressable>
      </View>

      {!isLoading && data && (
        <View key={data.id} style={styles.editor}>
          <TextInput
            style={styles.title}
            defaultValue={data.title}
            placeholder="Untitled"
            placeholderTextColor={Colors.muted}
            onChangeText={(title) => onEdit({ title })}
          />
          {audio.data && (
            <AudioChip
              key={`${audio.data.filename}:${audio.data.createdAt}`}
              uri={
                new File(Paths.document, "sessions", id, audio.data.filename)
                  .uri
              }
              filename={audio.data.filename}
              sizeBytes={audio.data.sizeBytes}
            />
          )}
          {audio.data &&
            audio.data.transcriptStatus !== "complete" &&
            transcripts.length === 0 &&
            (transcription === "running" ? (
              <Text style={styles.transcribeStatus}>Transcribing…</Text>
            ) : (
              <Pressable
                hitSlop={4}
                onPress={() => void transcribeSession(id)}
                style={({ pressed }) => pressed && styles.transcribePressed}
              >
                <Text style={styles.transcribeAction}>
                  {transcription === "failed"
                    ? "Transcription failed — tap to retry"
                    : "Tap to transcribe"}
                </Text>
              </Pressable>
            ))}
          {transcripts.length > 0 && (
            <View style={styles.transcript}>
              <Text style={styles.transcriptTitle}>Transcript</Text>
              <ScrollView style={styles.transcriptScroll} nestedScrollEnabled>
                {transcripts.map((segment) => (
                  <Text key={segment.id} style={styles.transcriptText}>
                    {segment.text}
                  </Text>
                ))}
              </ScrollView>
            </View>
          )}
          {!data.plainEditable && (
            <View style={styles.readOnlyChip}>
              <Ionicons
                name="lock-closed-outline"
                size={12}
                color={Colors.muted}
              />
              <Text style={styles.readOnlyLabel}>
                Formatted note — edit the body on desktop
              </Text>
            </View>
          )}
          <TextInput
            style={styles.body}
            multiline
            editable={data.plainEditable}
            defaultValue={data.noteText}
            placeholder="Start typing…"
            placeholderTextColor={Colors.muted}
            textAlignVertical="top"
            onChangeText={(body) => onEdit({ body })}
          />
        </View>
      )}

      {listening && (
        <ListeningSheet
          phase={recorder.phase}
          levels={recorder.levels}
          durationMs={recorder.durationMs}
          onStop={() => void handleStop()}
        />
      )}
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeArea: {
    flex: 1,
    backgroundColor: Colors.paper,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: Spacing.lg,
    paddingVertical: Spacing.md,
  },
  editor: {
    flex: 1,
  },
  title: {
    paddingHorizontal: Spacing.lg,
    fontSize: 24,
    fontWeight: "700",
    color: Colors.ink,
  },
  transcribeStatus: {
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.xs,
    fontSize: 12,
    color: Colors.muted,
  },
  transcribeAction: {
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.xs,
    fontSize: 12,
    fontWeight: "600",
    color: Colors.accent,
  },
  transcribePressed: {
    opacity: 0.6,
  },
  transcript: {
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
  },
  transcriptTitle: {
    fontSize: 13,
    fontWeight: "600",
    color: Colors.muted,
    marginBottom: Spacing.xs,
  },
  transcriptScroll: {
    maxHeight: 160,
  },
  transcriptText: {
    fontSize: 14,
    lineHeight: 20,
    color: Colors.ink,
    marginBottom: Spacing.sm,
  },
  readOnlyChip: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.xs,
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
  },
  readOnlyLabel: {
    fontSize: 12,
    color: Colors.muted,
  },
  body: {
    flex: 1,
    paddingHorizontal: Spacing.lg,
    paddingTop: Spacing.md,
    fontSize: 16,
    color: Colors.ink,
  },
});
