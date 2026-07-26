import { Ionicons } from "@expo/vector-icons";
import { useLocalSearchParams, useRouter } from "expo-router";
import { useEffect, useRef, useState } from "react";
import { Pressable, StyleSheet, Text, TextInput, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { useSessionRecorder } from "@/audio/use-session-recorder";
import { ListeningSheet } from "@/components/listening-sheet";
import { Colors, Radius, Spacing } from "@/constants/theme";
import { useSessionAudio } from "@/data/audio-catalog";
import {
  saveSessionNote,
  saveSessionTitle,
  useSessionDetail,
} from "@/data/session";

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

export default function NoteScreen() {
  const router = useRouter();
  const { id, listen } = useLocalSearchParams<{
    id: string;
    listen?: string;
  }>();
  const { data, isLoading } = useSessionDetail(id);
  const audio = useSessionAudio(id);
  const [listening, setListening] = useState(listen === "1");
  const recorder = useSessionRecorder(id, listening);

  const dataRef = useRef(data);
  dataRef.current = data;
  const draftRef = useRef<{ title?: string; body?: string }>({});
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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
      void saveSessionNote(id, {
        title: draft.title ?? current.title,
        bodyText: draft.body,
        bodyFormat: current.bodyFormat,
      });
    } else if (draft.title !== undefined) {
      void saveSessionTitle(id, draft.title);
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

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        <Pressable hitSlop={8} onPress={() => void handleBack()}>
          <Ionicons name="arrow-back" size={24} color={Colors.ink} />
        </Pressable>
        <Pressable hitSlop={8}>
          <Ionicons name="ellipsis-horizontal" size={22} color={Colors.ink} />
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
            <View style={styles.audioChip}>
              <Ionicons name="mic" size={14} color={Colors.accent} />
              <Text style={styles.audioLabel} numberOfLines={1}>
                {audio.data.filename} · {formatBytes(audio.data.sizeBytes)}
              </Text>
              {audio.data.transcriptStatus !== "complete" && (
                <Text style={styles.audioStatus}>Transcribes after sync</Text>
              )}
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
  audioChip: {
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
  audioLabel: {
    fontSize: 12,
    fontWeight: "600",
    color: Colors.ink,
    maxWidth: 180,
  },
  audioStatus: {
    fontSize: 12,
    color: Colors.muted,
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
